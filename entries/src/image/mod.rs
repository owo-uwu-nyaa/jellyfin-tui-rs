use std::{
    cmp::min,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use color_eyre::{Result, eyre::Context};
use image::DynamicImage;
use jellyfin::{JellyfinClient, items::ImageType};
use parking_lot::Mutex;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::{Image, Resize, picker::Picker, protocol::Protocol};
use sqlx::SqliteConnection;
use stats_data::Stats;
use tracing::{debug, instrument, trace};

use crate::image::{
    available::ImagesAvailable,
    cache::{ImageProtocolCache, ImageProtocolKey, ImageProtocolKeyRef},
};

pub mod available;
pub mod cache;
mod fetch;

struct ReadyImage {
    available: AtomicBool,
    image: Mutex<Option<Result<(DynamicImage, Rect)>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSize {
    pub p_width: u32,
    pub p_height: u32,
}

pub struct JellyfinImage {
    item_id: String,
    tag: String,
    image_type: ImageType,
    jellyfin: JellyfinClient,
    db: Arc<tokio::sync::Mutex<SqliteConnection>>,
    image: Option<(Protocol, ImageProtocolKey, Rect)>,
    size: Option<Rect>,
    available: ImagesAvailable,
    ready_image: Arc<ReadyImage>,
    cache: ImageProtocolCache,
    stats: Stats,
    picker: Arc<Picker>,
    loading: bool,
}

impl Drop for JellyfinImage {
    fn drop(&mut self) {
        if let Some((protocol,key,area)) = self.image.take(){
            self.cache.store(protocol, area, key);
        }
    }
}

impl FallibleWidget for JellyfinImage {
    #[instrument(skip_all, name = "render_image")]
    fn render_fallible(
        &mut self,
        mut area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> color_eyre::Result<()> {
        if let Some(old_area) = self.size.replace(area)
            && (old_area.width != area.width || old_area.height != area.height)
        {
            self.image = None;
        }
        if let Some((image, size)) = self.get_image()? {
            trace!("received_image");
            trace!("area: {area:?}, size: {size:?}");
            area.x += (area.width - size.width) / 2;
            area.y += (area.height - size.height) / 2;
            area.width = size.width;
            area.height = size.height;
            trace!("final area: {area:?}");
            Image::new(image).render(area, buf)
        }
        Ok(())
    }
}

impl JellyfinImage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        item_id: String,
        tag: String,
        image_type: ImageType,
        jellyfin: JellyfinClient,
        db: Arc<tokio::sync::Mutex<SqliteConnection>>,
        available: ImagesAvailable,
        cache: ImageProtocolCache,
        picker: Arc<Picker>,
        stats: Stats,
    ) -> Self {
        Self {
            item_id,
            tag,
            image_type,
            jellyfin,
            db,
            image: None,
            size: None,
            available,
            ready_image: Arc::new(ReadyImage {
                available: AtomicBool::new(false),
                image: Mutex::new(None),
            }),
            cache,
            picker,
            loading: false,
            stats,
        }
    }

    /// size must be set before calling this
    #[instrument(skip_all)]
    fn get_image(&mut self) -> Result<Option<(&Protocol, Rect)>> {
        if self.image.is_some() {
            Ok(self.image.as_ref().map(|(p, _, s)| (p, *s)))
        } else if let Some(size) = self.size {
            let p_height = (size.height as u32) * (self.picker.font_size().1 as u32);
            let p_width = (size.width as u32) * (self.picker.font_size().0 as u32);
            if self.loading {
                if self.ready_image.available.swap(false, Ordering::SeqCst) {
                    self.loading = false;
                    let (image, new_size) = self
                        .ready_image
                        .image
                        .lock()
                        .take()
                        .expect("available is already set")?;
                    if size.width != new_size.width || size.height != new_size.height {
                        debug!("size mismatch, retrying");
                        self.loading = false;
                        self.get_image()
                    } else {
                        let width = min(
                            size.width as u32,
                            image.width().div_ceil(self.picker.font_size().0 as u32),
                        ) as u16;
                        let height = min(
                            size.height as u32,
                            image.height().div_ceil(self.picker.font_size().1 as u32),
                        ) as u16;
                        let image_size = Rect {
                            x: 0,
                            y: 0,
                            width,
                            height,
                        };
                        let image = self
                            .picker
                            .new_protocol(image, image_size, Resize::Fit(None))
                            .context("generating protocol")?;
                        let (image, _, _) = self.image.insert((
                            image,
                            ImageProtocolKey {
                                image_type: self.image_type,
                                item_id: self.item_id.clone(),
                                tag: self.tag.clone(),
                                size: ImageSize { p_width, p_height },
                            },
                            image_size,
                        ));
                        Ok(Some((image, image_size)))
                    }
                } else {
                    Ok(None)
                }
            } else {
                let cached = self.cache.remove(&ImageProtocolKeyRef::new(
                    self.image_type,
                    &self.item_id,
                    &self.tag,
                    ImageSize { p_width, p_height },
                ));
                if let Some((image, size)) = cached {
                    self.stats
                        .memory_image_cache_hits
                        .fetch_add(1, Ordering::Relaxed);
                    let (image, _, _) = self.image.insert((
                        image,
                        ImageProtocolKey {
                            image_type: self.image_type,
                            item_id: self.item_id.clone(),
                            tag: self.tag.clone(),
                            size: ImageSize { p_width, p_height },
                        },
                        size,
                    ));
                    Ok(Some((image, size)))
                } else {
                    tokio::spawn(fetch::get_image(
                        ImageProtocolKey {
                            image_type: self.image_type,
                            item_id: self.item_id.clone(),
                            tag: self.tag.clone(),
                            size: ImageSize { p_width, p_height },
                        },
                        self.ready_image.clone(),
                        self.available.clone(),
                        self.db.clone(),
                        self.jellyfin.clone(),
                        size,
                        self.stats.clone(),
                    ));
                    self.loading = true;
                    Ok(None)
                }
            }
        } else {
            panic!("size is not set")
        }
    }
}
