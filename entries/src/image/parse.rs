use std::{
    io::Cursor,
    sync::{Weak, atomic::Ordering},
};

use bytes::Bytes;
use color_eyre::eyre::Context;
use image::ImageReader;
use tracing::{instrument, trace, warn};

use crate::image::{
    available::ImagesAvailableInner,
    cache::{ImageProtocolCache, ImageProtocolKey},
    state::{ImageStateInner, ImageStateInnerState},
};

#[instrument(skip_all)]
pub(super) fn parse_image(
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
