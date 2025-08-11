use std::{
    cmp::min, collections::HashMap, fmt::Debug, future::Future, io::Cursor, ops::DerefMut, sync::{
        atomic::{AtomicBool, Ordering}, Arc, Weak
    }, task::{self, Poll, Waker}
};

use bytes::Bytes;
use color_eyre::eyre::Context;
use either::Either;
use image::{DynamicImage, ImageReader};
use jellyfin::{
    AuthStatus, JellyfinClient,
    image::{GetImage, GetImageQuery},
    items::ImageType,
    sha::ShaImpl,
};
use log::trace;
use parking_lot::Mutex;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui_image::{
    FilterType, Resize, ResizeEncodeRender, picker::Picker, protocol::StatefulProtocol,
};
use sqlx::{SqlitePool, query, query_scalar};
use tokio::select;
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{debug, info, instrument, warn};

use crate::{
    Result,
    entry::{IMAGE_WIDTH, image_height},
};

#[instrument]
pub async fn clean_images(db: SqlitePool) -> Result<()> {
    let res = query!("delete from image_cache where (added+7*24*60*60)<unixepoch()")
        .execute(&db)
        .await
        .context("deleting old images from cache")?;
    if res.rows_affected() > 0 {
        info!("removed {} images from cache", res.rows_affected());
    }
    Ok(())
}

struct ImagesAvailableInner {
    available: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl ImagesAvailableInner {
    #[instrument(level = "trace", skip_all)]
    fn wake(&self) {
        trace!("images available");
        if !self.available.load(Ordering::SeqCst) && !self.available.swap(true, Ordering::SeqCst) {
            if let Some(waker) = self.waker.lock().take() {
                trace!("waking");
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
                waker: Mutex::new(None),
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

    #[instrument(level = "trace", skip_all)]
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        if self.inner.available.swap(false, Ordering::SeqCst) {
            trace!("awakened");
            Poll::Ready(())
        } else {
            let mut waker = self.inner.waker.lock();
            if self.inner.available.swap(false, Ordering::SeqCst) {
                trace!("awakened after lock");
                Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                trace!("sleeping");
                Poll::Pending
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ImageProtocolKey {
    tag: String,
    item_id: String,
    image_type: ImageType,
}

type CachedImage = Either<(StatefulProtocol, u16), DynamicImage>;

#[derive(Clone)]
pub struct ImageProtocolCache {
    protocols: Arc<Mutex<HashMap<ImageProtocolKey, CachedImage>>>,
}

impl ImageProtocolCache {
    #[instrument(level = "trace", skip_all)]
    fn store_protocol(&self, protocol: StatefulProtocol, key: ImageProtocolKey, width: u16) {
        trace!("storing image protocol in cache");
        self.protocols
            .lock()
            .insert(key, Either::Left((protocol, width)));
    }
    #[instrument(level = "trace", skip_all)]
    fn store_image(&self, image: DynamicImage, key: ImageProtocolKey) {
        let mut map = self.protocols.lock();
        if let std::collections::hash_map::Entry::Vacant(entry) = map.entry(key) {
            trace!("storing image in cache");
            entry.insert(Either::Right(image));
        }
    }
    pub fn new() -> Self {
        Self {
            protocols: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

enum ImageStateInnerState {
    Lazy {
        get_image: GetImage,
        db: SqlitePool,
        tag: String,
        item_id: String,
        image_type: ImageType,
        cancel: CancellationToken,
    },
    Invalid,
    ImageReady(DynamicImage, ImageProtocolKey),
    Image(StatefulProtocol, ImageProtocolKey, u16),
}


impl Default for ImageStateInnerState {
    fn default() -> Self {
        Self::Invalid
    }
}


struct ImageStateInner {
    _cancel_fetch: Option<DropGuard>,
    ready: AtomicBool,
    cache: ImageProtocolCache,
    value: Mutex<ImageStateInnerState>,
}

impl Drop for ImageStateInner {
    fn drop(&mut self) {
        match std::mem::take(self.value.get_mut()) {
            ImageStateInnerState::ImageReady(image, key) => self.cache.store_image(image, key),
            ImageStateInnerState::Image(protocol, key, width) => {
                self.cache.store_protocol(protocol, key, width)
            }
            _ => {}
        }
    }
}

pub struct JellyfinImageState {
    inner: Arc<ImageStateInner>,
}

#[instrument(skip_all)]
fn parse_image(
    val: Bytes,
    out: &Weak<ImageStateInner>,
    wake: &ImagesAvailableInner,
    key: ImageProtocolKey,
    cache: ImageProtocolCache,
) {
    trace!("parsing image");
    let reader = match ImageReader::new(Cursor::new(val)).with_guessed_format() {
        Ok(reader) => reader,
        Err(e) => {
            warn!("error guessing image format: {e:?}");
            if let Some(out) = out.upgrade() {
                *out.value.lock() = ImageStateInnerState::Invalid;
            };
            return;
        }
    };
    match reader.decode().context("decoding image") {
        Ok(image) => {
            trace!("image parsed");
            if let Some(out) = out.upgrade() {
                *out.value.lock() = ImageStateInnerState::ImageReady(image, key);
                out.ready.store(true, Ordering::SeqCst);
                wake.wake();
            } else {
                cache.store_image(image, key);
            }
        }
        Err(e) => {
            warn!("parsing error: {e:?}");
            if let Some(out) = out.upgrade() {
                *out.value.lock() = ImageStateInnerState::Invalid;
            };
        }
    };
}

#[instrument(skip_all)]
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
            trace!("db access cancelled");
            return Ok(None);
        }
        res = query => {
            if let Some(image)=res.context("asking db for cached image")?{
                debug!("image loaded from cache");
                return Ok(Some(image.into()))
            }
        }
    };
    debug!("requesting image");
    let query = GetImageQuery {
        tag: Some(tag),
        format: Some("webp"),
    };
    let image = get_image.get(&query);
    let image = select! {
        biased;
        res = image => {
            trace!("image received");
            res.context("fetching image")?
        }
        _ = cancel.cancelled() => {
            trace!("fetch cancelled");
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
    trace!("image stored");
    Ok(if cancel.is_cancelled() {
        trace!("image request cancelled after store");
        None
    } else {
        Some(image)
    })
}

#[instrument(skip_all)]
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
    cache: ImageProtocolCache,
) {
    match do_fetch_image(get_image, &db, &tag, &item_id, image_type.name(), &cancel).await {
        Ok(Some(image)) => {
            rayon::spawn(move || {
                parse_image(
                    image,
                    &out,
                    &wake,
                    ImageProtocolKey {
                        tag,
                        item_id,
                        image_type,
                    },
                    cache,
                )
            });
        }
        Ok(None) => {}
        Err(e) => {
            warn!("error fetching image: {e:?}");
            if let Some(out) = out.upgrade() {
                trace!("output dropped");
                *out.value.lock() = ImageStateInnerState::Invalid
            }
        }
    }
}

impl JellyfinImageState {
    pub fn new(
        client: &JellyfinClient<impl AuthStatus, impl ShaImpl>,
        db: SqlitePool,
        tag: String,
        item_id: String,
        image_type: ImageType,
        cache: ImageProtocolCache,
    ) -> Self {
        let key = ImageProtocolKey {
            tag,
            item_id,
            image_type,
        };
        let cached = cache.protocols.lock().remove(&key);
        if let Some(cached) = cached {
            trace!("got image from cache");
            let state = match cached {
                Either::Left((protocol, width)) => {
                    ImageStateInnerState::Image(protocol, key, width)
                }
                Either::Right(image) => ImageStateInnerState::ImageReady(image, key),
            };
            Self {
                inner: Arc::new(ImageStateInner {
                    _cancel_fetch: None,
                    ready: true.into(),
                    cache,
                    value: Mutex::new(state),
                }),
            }
        } else {
            let ImageProtocolKey {
                tag,
                item_id,
                image_type,
            } = key;
            let get_image = client.prepare_get_image(&item_id, image_type);
            let cancel = CancellationToken::new();
            Self {
                inner: Arc::new(ImageStateInner {
                    _cancel_fetch: cancel.clone().drop_guard().into(),
                    ready: true.into(),
                    value: Mutex::new(ImageStateInnerState::Lazy {
                        get_image,
                        db,
                        tag,
                        item_id,
                        image_type,
                        cancel,
                    }),
                    cache,
                }),
            }
        }
    }
    #[instrument(skip_all, name = "prefetch_image")]
    pub fn prefetch(&mut self, availabe: &ImagesAvailable) {
        if self.inner.ready.load(Ordering::SeqCst) {
            let mut value_ref = self.inner.value.lock();
            let value = std::mem::take(value_ref.deref_mut());
            match value {
                ImageStateInnerState::Invalid => panic!("image in invalid state"),
                ImageStateInnerState::Lazy {
                    get_image,
                    db,
                    tag,
                    item_id,
                    image_type,
                    cancel,
                } => {
                    self.inner.ready.store(false, Ordering::SeqCst);
                    tokio::spawn(fetch_image(
                        get_image,
                        db,
                        tag,
                        item_id,
                        image_type,
                        cancel,
                        Arc::downgrade(&self.inner),
                        availabe.inner.clone(),
                        self.inner.cache.clone(),
                    ));
                }
                val @ ImageStateInnerState::Image(_, _, _)
                | val @ ImageStateInnerState::ImageReady(_, _) => {
                    *value_ref = val;
                }
            }
        }
    }
}

#[instrument(skip_all)]
fn resize_image(
    resize: Resize,
    area: Rect,
    out: &Weak<ImageStateInner>,
    wake: &ImagesAvailableInner,
) {
    trace!("resizing image");
    if let Some(out) = out.upgrade() {
        let mut value = out.value.lock();
        if let ImageStateInnerState::Image(protocol, _, _) = value.deref_mut() {
            protocol.resize_encode(&resize, area);
            trace!("resized");
        } else {
            *value = ImageStateInnerState::Invalid;
            panic!("tried to resize invalid state");
        }
        out.ready.store(true, Ordering::SeqCst);
        wake.wake();
    } else {
        trace!("cancelled");
    }
}

pub struct JellyfinImage {
    resize: Resize,
}

impl Default for JellyfinImage {
    fn default() -> Self {
        Self {
            resize: Resize::Scale(FilterType::Triangle.into()),
        }
    }
}

impl JellyfinImage {
    #[allow(unused)]
    pub fn resize(self, resize: Resize) -> JellyfinImage {
        JellyfinImage { resize }
    }

    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    fn render_image_inner(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        mut image: StatefulProtocol,
        key: ImageProtocolKey,
        state_mut: &mut ImageStateInnerState,
        state: &JellyfinImageState,
        availabe: &ImagesAvailable,
        width: u16,
    ) {
        let [area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);
        if let Some(area) = image.needs_resize(&self.resize, area) {
            trace!("image needs resize");
            state.inner.ready.store(false, Ordering::SeqCst);
            *state_mut = ImageStateInnerState::Image(image, key, width);
            let resize = self.resize;
            let out = Arc::downgrade(&state.inner);
            let wake = availabe.inner.clone();
            rayon::spawn(move || resize_image(resize, area, &out, &wake));
        } else {
            image.render(area, buf);
            *state_mut = ImageStateInnerState::Image(image, key, width);
        }
    }

    #[instrument(skip_all, name = "render_image")]
    pub fn render(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut JellyfinImageState,
        availabe: &ImagesAvailable,
        picker: &Picker,
    ) {
        if state.inner.ready.load(Ordering::SeqCst) {
            let mut value_ref = state.inner.value.lock();
            let value = std::mem::take(value_ref.deref_mut());
            match value {
                ImageStateInnerState::Invalid => panic!("image in invalid state"),
                ImageStateInnerState::ImageReady(dynamic_image, key) => {
                    trace!("image ready");
                    let image_height = image_height(picker.font_size());
                    let height = image_height * picker.font_size().1;
                    let height: f64 = height.into();
                    let width =
                        (height / (dynamic_image.height() as f64)) * (dynamic_image.width() as f64);
                    let width = width / (picker.font_size().0 as f64);
                    let width = min(width.ceil() as u16, IMAGE_WIDTH);
                    let image = picker.new_resize_protocol(dynamic_image);
                    self.render_image_inner(
                        area,
                        buf,
                        image,
                        key,
                        value_ref.deref_mut(),
                        state,
                        availabe,
                        width,
                    );
                }
                ImageStateInnerState::Image(image, key, width) => {
                    self.render_image_inner(
                        area,
                        buf,
                        image,
                        key,
                        value_ref.deref_mut(),
                        state,
                        availabe,
                        width,
                    );
                }
                ImageStateInnerState::Lazy {
                    get_image,
                    db,
                    tag,
                    item_id,
                    image_type,
                    cancel,
                } => {
                    state.inner.ready.store(false, Ordering::SeqCst);
                    tokio::spawn(fetch_image(
                        get_image,
                        db,
                        tag,
                        item_id,
                        image_type,
                        cancel,
                        Arc::downgrade(&state.inner),
                        availabe.inner.clone(),
                        state.inner.cache.clone(),
                    ));
                }
            }
        }
    }
}
