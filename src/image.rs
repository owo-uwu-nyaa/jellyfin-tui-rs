use std::{
    future::Future,
    io::Cursor,
    ops::DerefMut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{self, Poll, Waker}, time::Duration,
};

use bytes::Bytes;
use color_eyre::{
    eyre::{eyre, Context},
    Report,
};
use image::{DynamicImage, ImageFormat, ImageReader};
use jellyfin::{
    image::{GetImage, GetImageQuery},
    items::ImageType,
    sha::Sha256,
    AuthStatus, JellyfinClient,
};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, FilterType, Resize};
use sqlx::{query, query_scalar, SqlitePool};
use tokio::select;
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{error, info, instrument};

use crate::Result;

#[instrument(skip_all)]
pub async fn clean_image_cache(db: SqlitePool) {
    let mut interval = tokio::time::interval(Duration::from_secs(60*60));
    let err = loop {
        select! {
            biased;
            _ = db.close_event() => {
                return
            }
            _ = interval.tick() => {}
        }

        match query!("delete from image_cache where (added+7*24*60*60)<unixepoch()")
            .execute(&db)
            .await
            .context("deleting old images from cache")
        {
            Err(e) => break e,
            Ok(res) => {
                if res.rows_affected() > 0{
                    info!("removed {} images from cache", res.rows_affected());
                }
            }
        }
    };
    error!("Error cleaning image cache: {err:?}");
}

struct ImagesAvailableInner {
    available: AtomicBool,
    waker: parking_lot::Mutex<Option<Waker>>,
}

impl ImagesAvailableInner {
    fn wake(&self) {
        if !self.available.load(Ordering::SeqCst) && !self.available.swap(true, Ordering::SeqCst) {
            if let Some(waker) = self.waker.lock().take() {
                waker.wake();
            }
        }
    }
}

pub struct ImagesAvailable {
    inner: Arc<ImagesAvailableInner>,
}

impl ImagesAvailable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ImagesAvailableInner {
                available: false.into(),
                waker: parking_lot::Mutex::new(None),
            }),
        }
    }
    pub fn wait_available(&self) -> ImagesAvailableFuture<'_> {
        ImagesAvailableFuture { inner: &self.inner }
    }
}

pub struct ImagesAvailableFuture<'a> {
    inner: &'a ImagesAvailableInner,
}

impl Future for ImagesAvailableFuture<'_> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        if self.inner.available.swap(false, Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            let mut waker = self.inner.waker.lock();
            if self.inner.available.swap(false, Ordering::SeqCst) {
                Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

enum ImageStateInnerState {
    Invalid,
    ImageReady(DynamicImage),
    Image(StatefulProtocol),
    Err(Report),
    ErrTaken,
}

struct ImageStateInner {
    _cancel_fetch: DropGuard,
    ready: AtomicBool,
    value: parking_lot::Mutex<ImageStateInnerState>,
}

pub struct JellyfinImageState {
    inner: Arc<ImageStateInner>,
}

fn parse_image(val: Bytes, out: &Weak<ImageStateInner>, wake: &ImagesAvailableInner) {
    let out = if let Some(out) = out.upgrade() {
        out
    } else {
        return;
    };
    let mut reader = ImageReader::new(Cursor::new(val));
    reader.set_format(ImageFormat::WebP);
    *out.value.lock() = match reader.decode().context("decoding image") {
        Ok(image) => ImageStateInnerState::ImageReady(image),
        Err(e) => ImageStateInnerState::Err(e),
    };
    out.ready.store(true, Ordering::SeqCst);
    wake.wake();
}

async fn do_fetch_image(
    get_image: GetImage,
    db: &SqlitePool,
    tag: &str,
    item_id: &str,
    image_type: &str,
    cancel: &CancellationToken,
) -> Result<Option<Bytes>> {
    let query = query_scalar!(
        "select val from image_cache where item_id = ? and image_type = ? and tag = ?",
        item_id,
        image_type,
        tag
    )
    .fetch_optional(db);
    select! {
        biased;
        _ = cancel.cancelled() => {
            return Ok(None);
        }
        res = query => {
            if let Some(image)=res.context("asking db for cached image")?{
                return Ok(Some(image.into()))
            }
        }
    };
    let query = GetImageQuery {
        tag: Some(tag),
        format: Some("webp"),
    };
    let image = get_image.get(&query);
    let image = select! {
        biased;
        res = image => {
            res.context("fetching image")?
        }
        _ = cancel.cancelled() => {
            return Ok(None);
        }
    };
    let image_ref = image.as_ref();
    query!(
        "insert into image_cache (item_id, image_type, tag, val) values (?, ?, ?, ?)",
        item_id,
        image_type,
        tag,
        image_ref
    )
    .execute(db)
    .await
    .context("storing image in cache")?;
    Ok(if cancel.is_cancelled() {
        None
    } else {
        Some(image)
    })
}

#[allow(clippy::too_many_arguments)]
async fn fetch_image(
    get_image: GetImage,
    db: SqlitePool,
    tag: String,
    item_id: String,
    image_type: ImageType,
    cancel: CancellationToken,
    out: Weak<ImageStateInner>,
    wake: Arc<ImagesAvailableInner>,
) {
    match do_fetch_image(get_image, &db, &tag, &item_id, image_type.name(), &cancel).await {
        Ok(Some(image)) => {
            rayon::spawn(move || parse_image(image, &out, &wake));
        }
        Ok(None) => {}
        Err(e) => {
            if let Some(out) = out.upgrade() {
                *out.value.lock() = ImageStateInnerState::Err(e)
            }
        }
    }
}

impl JellyfinImageState {
    pub fn new(
        client: &JellyfinClient<impl AuthStatus, impl Sha256>,
        db: SqlitePool,
        wake: &ImagesAvailable,
        tag: String,
        item_id: String,
        image_type: ImageType,
    ) -> Self {
        let get_image = client.prepare_get_image(&item_id, image_type);
        let cancel = CancellationToken::new();
        let inner = Arc::new(ImageStateInner {
            _cancel_fetch: cancel.clone().drop_guard(),
            value: parking_lot::Mutex::new(ImageStateInnerState::Invalid),
            ready: false.into(),
        });
        tokio::spawn(fetch_image(
            get_image,
            db,
            tag,
            item_id,
            image_type,
            cancel,
            Arc::downgrade(&inner),
            wake.inner.clone(),
        ));
        Self { inner }
    }
}

fn resize_image(
    resize: Resize,
    area: Rect,
    out: &Weak<ImageStateInner>,
    wake: &ImagesAvailableInner,
) {
    if let Some(out) = out.upgrade() {
        let mut value = out.value.lock();
        if let ImageStateInnerState::Image(protocol) = value.deref_mut() {
            protocol.resize_encode(&resize, protocol.background_color(), area);
        } else {
            *value = ImageStateInnerState::Err(eyre!("tried to resize invalid state"))
        }
        out.ready.store(true, Ordering::SeqCst);
        wake.wake();
    }
}

pub struct JellyfinImage {
    resize: Resize,
}

impl Default for JellyfinImage {
    fn default() -> Self {
        Self {
            resize: Resize::Scale(FilterType::CatmullRom.into()),
        }
    }
}

impl JellyfinImage {
    #[allow(unused)]
    pub fn resize(self, resize: Resize) -> JellyfinImage {
        JellyfinImage { resize }
    }

    fn render_image(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        mut image: StatefulProtocol,
        state_mut: &mut ImageStateInnerState,
        state: &JellyfinImageState,
        availabe: &ImagesAvailable,
    ) {
        if let Some(area) = image.needs_resize(&self.resize, area) {
            *state_mut = ImageStateInnerState::Image(image);
            state.inner.ready.store(false, Ordering::SeqCst);
            let resize = self.resize;
            let out = Arc::downgrade(&state.inner);
            let wake = availabe.inner.clone();
            rayon::spawn(move || resize_image(resize, area, &out, &wake));
        } else {
            image.render(area, buf);
            *state_mut = ImageStateInnerState::Image(image);
        }
    }

    pub fn render(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut JellyfinImageState,
        availabe: &ImagesAvailable,
        picker: &Picker,
    ) -> Result<()> {
        if state.inner.ready.load(Ordering::SeqCst) {
            let mut value_ref = state.inner.value.lock();
            let value = std::mem::replace(value_ref.deref_mut(), ImageStateInnerState::Invalid);
            match value {
                ImageStateInnerState::Invalid => Err(eyre!("image in invalid state")),
                ImageStateInnerState::ImageReady(dynamic_image) => {
                    let image = picker.new_resize_protocol(dynamic_image);
                    self.render_image(area, buf, image, value_ref.deref_mut(), state, availabe);
                    Ok(())
                }
                ImageStateInnerState::Image(image) => {
                    self.render_image(area, buf, image, value_ref.deref_mut(), state, availabe);
                    Ok(())
                }
                ImageStateInnerState::Err(report) => {
                    *value_ref = ImageStateInnerState::ErrTaken;
                    Err(report)
                }
                ImageStateInnerState::ErrTaken => {
                    *value_ref = ImageStateInnerState::ErrTaken;
                    Err(eyre!("Error already returned"))
                }
            }
        } else {
            Ok(())
        }
    }
}
