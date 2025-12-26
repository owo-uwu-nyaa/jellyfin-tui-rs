use std::{
    io::Cursor,
    sync::{Arc, atomic::Ordering::{Relaxed, SeqCst}},
};

use crate::image::{ReadyImage, available::ImagesAvailable, cache::ImageProtocolKey};
use bytes::Bytes;
use color_eyre::{Result, eyre::Context};
use image::{DynamicImage, ImageReader};
use jellyfin::{JellyfinClient, image::GetImageQuery};
use ratatui::layout::Rect;
use sqlx::SqliteConnection;
use stats_data::Stats;
use std::ops::DerefMut;
use tracing::{debug, instrument};

#[instrument(skip_all)]
pub async fn get_image(
    key: ImageProtocolKey,
    ready_image: Arc<ReadyImage>,
    available: ImagesAvailable,
    db: Arc<tokio::sync::Mutex<SqliteConnection>>,
    jellyfin: JellyfinClient,
    size: Rect,
    stats: Stats,
) {
    match {
        let image_type = key.image_type.name();
        let item_id = &key.item_id;
        let tag = &key.tag;
        sqlx::query_scalar!(
            "select val from image_cache where
             item_id = ? and
             image_type = ? and
             tag = ? and
             size_x = ? and
             size_y = ?",
            item_id,
            tag,
            image_type,
            key.size.p_width,
            key.size.p_height
        )
        .fetch_optional(db.lock().await.deref_mut())
        .await
    }
    .context("Get image from cache")
    .transpose()
    {
        Some(Ok(val)) => {
            stats.db_image_cache_hits.fetch_add(1, Relaxed);
            rayon::spawn(move || parse_image(ready_image, available, &val, size));
        }
        Some(Err(e)) => {
            *ready_image.image.lock() = Some(Err(e));
            ready_image.available.store(true, SeqCst);
            available.inner.wake();
        }
        None => {
            stats.image_fetches.fetch_add(1, Relaxed);
            match fetch_image(key, jellyfin, db).await {
                Ok(image) => rayon::spawn(move || parse_image(ready_image, available, &image, size)),
                Err(e) => {
                    *ready_image.image.lock() = Some(Err(e));
                    ready_image.available.store(true, SeqCst);
                    available.inner.wake();
                }
            }
        },
    }
}

#[instrument(skip_all)]
async fn fetch_image(
    key: ImageProtocolKey,
    jellyfin: JellyfinClient,
    db: Arc<tokio::sync::Mutex<SqliteConnection>>,
) -> Result<Bytes> {
    let image = jellyfin
        .get_image(
            &key.item_id,
            key.image_type,
            &GetImageQuery {
                tag: Some(&key.tag),
                format: Some("Webp"),
                max_width: Some(key.size.p_width),
                max_height: Some(key.size.p_height),
            },
        )
        .await?;
    let val: &[u8] = &image;
    let image_type = key.image_type.name();
    sqlx::query!("insert into image_cache (item_id, image_type, tag, size_x, size_y, val) values (?,?,?,?,?,?)",
        key.item_id,image_type, key.tag, key.size.p_width, key.size.p_height,val
    ).execute(db.lock().await.deref_mut()).await?;
    Ok(image)
}

fn parse_image_inner(val: &[u8]) -> Result<DynamicImage> {
    ImageReader::new(Cursor::new(val))
        .with_guessed_format()
        .context("detecting image type")?
        .decode()
        .context("parsing image")
}

#[instrument(skip_all)]
fn parse_image(ready_image: Arc<ReadyImage>, available: ImagesAvailable, val: &[u8], size: Rect) {
    *ready_image.image.lock() = Some(parse_image_inner(val).map(move |p| (p, size)));
    debug!("Image ready");
    ready_image.available.store(true, SeqCst);
    available.inner.wake();
}
