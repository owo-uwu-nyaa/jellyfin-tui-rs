use std::sync::{Arc, Weak};

use bytes::Bytes;
use color_eyre::{Result, eyre::Context};
use jellyfin::{image::GetImage, items::ImageType};
use sqlx::{SqlitePool, query, query_scalar};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, trace, warn};

use crate::image::{
    available::ImagesAvailableInner,
    cache::{ImageProtocolCache, ImageProtocolKey},
    parse::parse_image,
    state::{ImageStateInner, ImageStateInnerState},
};

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
    let image = get_image.get();
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
pub(super) async fn fetch_image(
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
                    ImageProtocolKey::new(image_type, item_id, tag),
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
