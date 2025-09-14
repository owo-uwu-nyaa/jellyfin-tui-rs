use std::{
    ops::DerefMut,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use color_eyre::Result;
use either::Either;
use image::DynamicImage;
use jellyfin::{
    AuthStatus, JellyfinClient,
    image::{GetImage, GetImageQuery},
    items::ImageType,
};
use parking_lot::Mutex;
use ratatui_image::protocol::StatefulProtocol;
use sqlx::SqlitePool;
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{instrument, trace};

use crate::image::{
    available::ImagesAvailable,
    cache::{ImageProtocolCache, ImageProtocolKey},
    fetch::fetch_image,
};

pub(super) enum ImageStateInnerState {
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

pub(super) struct ImageStateInner {
    _cancel_fetch: Option<DropGuard>,
    pub(super) ready: AtomicBool,
    pub(super) cache: ImageProtocolCache,
    pub(super) value: Mutex<ImageStateInnerState>,
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
    pub(super) inner: Arc<ImageStateInner>,
}

impl JellyfinImageState {
    pub fn new(
        client: &JellyfinClient<impl AuthStatus>,
        db: SqlitePool,
        tag: String,
        item_id: String,
        image_type: ImageType,
        cache: ImageProtocolCache,
    ) -> Result<Self> {
        let key = ImageProtocolKey::new(image_type, item_id, tag);
        let cached = cache.remove(&key);
        let res = if let Some(cached) = cached {
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
            let get_image = client.prepare_get_image(
                &item_id,
                image_type,
                &GetImageQuery {
                    tag: Some(&tag),
                    format: Some("webp"),
                },
            )?;
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
        };
        Ok(res)
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
