use std::{
    io::Cursor,
    sync::{Arc, atomic::Ordering::Relaxed},
};

use bytes::Bytes;
use color_eyre::{Result, eyre::Context};
use image::{DynamicImage, ImageReader};
use jellyfin::{JellyfinClient, image::GetImageQuery};
use ratatui::layout::Size;
use sqlx::SqliteConnection;
use stats_data::Stats;
use std::ops::DerefMut;
use tracing::instrument;

use crate::{ImageSize, cache::ImageProtocolKey};

pub struct ParsedImage {
    pub(crate) image: DynamicImage,
    pub(crate) size: Size,
    pub(crate) image_size: ImageSize,
}

#[instrument(skip_all)]
pub async fn get_image(
    key: ImageProtocolKey,
    db: Arc<tokio::sync::Mutex<SqliteConnection>>,
    jellyfin: JellyfinClient,
    size: Size,
    stats: Stats,
) -> Result<ParsedImage> {
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
            image_type,
            tag,
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
            let image = parse_image(val.into()).await?;
            Ok(ParsedImage {
                image,
                size,
                image_size: key.size,
            })
        }
        Some(Err(e)) => Err(e),
        None => {
            stats.image_fetches.fetch_add(1, Relaxed);
            let image_size = key.size;
            match fetch_image(key, jellyfin, db).await {
                Ok(image) => {
                    let image = parse_image(image).await?;
                    Ok(ParsedImage {
                        image,
                        size,
                        image_size,
                    })
                }
                Err(e) => Err(e),
            }
        }
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

#[instrument(skip_all)]
async fn parse_image(val: Bytes) -> Result<DynamicImage> {
    let (send, recv) = tokio::sync::oneshot::channel();

    rayon::spawn(move || {
        let image = ImageReader::new(Cursor::new(val))
            .with_guessed_format()
            .context("detecting image type")
            .and_then(|v| v.decode().context("parsing image"));
        let _ = send.send(image);
    });
    recv.await.context("image parser did not finish")?
}
