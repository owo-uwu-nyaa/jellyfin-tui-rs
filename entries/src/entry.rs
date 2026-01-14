use std::{borrow::Cow, fmt::Debug, sync::Arc};

use jellyfin::{
    JellyfinClient,
    image::select_images,
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use ratatui::{
    layout::Rect,
    style::Color,
    text::Span,
    widgets::{Block, BorderType, Paragraph, Widget},
};
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::{FontSize, picker::Picker};
use sqlx::SqliteConnection;
use stats_data::Stats;
use tracing::instrument;

use crate::image::{JellyfinImage, available::ImagesAvailable, cache::ImageProtocolCache};
use color_eyre::Result;

pub struct Entry {
    image: Option<JellyfinImage>,
    title: String,
    subtitle: Option<String>,
    inner: EntryInner,
    watch_status: Option<Cow<'static, str>>,
    pub border_type: BorderType,
}

impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("title", &self.title)
            .field("subtitle", &self.subtitle)
            .field("watch_status", &self.watch_status)
            .finish_non_exhaustive()
    }
}

pub const IMAGE_WIDTH: u16 = 32;
pub fn image_height(font: FontSize) -> u16 {
    let width = IMAGE_WIDTH * font.0;
    let width: f64 = width.into();
    let height = (width / 16.0) * 9.0;
    let height = height / f64::from(font.1);
    height.ceil() as u16
}

pub const ENTRY_WIDTH: u16 = IMAGE_WIDTH + 2;

pub fn entry_height(font: FontSize) -> u16 {
    image_height(font) + 2
}

impl FallibleWidget for Entry {
    #[instrument(skip_all, name = "render_entry")]
    fn render_fallible(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> color_eyre::Result<()> {
        let mut outer = Block::bordered()
            .border_type(self.border_type)
            .title_top(self.title.as_str());
        if let Some(subtitle) = &self.subtitle {
            outer = outer.title_bottom(subtitle.as_str());
        }
        let inner = outer.inner(area);
        outer.render(area, buf);
        if let Some(watch_status) = self.watch_status.as_ref() {
            Paragraph::new(Span::styled(watch_status.clone(), Color::LightBlue))
                .right_aligned()
                .render(
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
        }
        if let Some(image) = &mut self.image {
            image.render_fallible(inner, buf)?;
        }
        Ok(())
    }
}

impl Entry {
    pub fn inner(&self) -> &EntryInner {
        &self.inner
    }

    pub fn new(
        image: Option<JellyfinImage>,
        title: String,
        subtitle: Option<String>,
        inner: EntryInner,
        watch_status: Option<Cow<'static, str>>,
    ) -> Self {
        Self {
            image,
            title,
            subtitle,
            inner,
            watch_status,
            border_type: BorderType::Rounded,
        }
    }

    pub fn from_media_item(
        item: MediaItem,
        jellyfin: &JellyfinClient,
        db: &Arc<tokio::sync::Mutex<SqliteConnection>>,
        cache: &ImageProtocolCache,
        availabe: &ImagesAvailable,
        picker: &Arc<Picker>,
        stats: &Stats,
    ) -> Result<Option<Self>> {
        let (title, subtitle) = match &item.item_type {
            ItemType::Movie => (item.name.clone(), None),
            ItemType::Episode {
                season_id: _,
                season_name: _,
                series_id: _,
                series_name,
            } => (series_name.clone(), item.name.clone().into()),
            ItemType::Season {
                series_id: _,
                series_name,
            } => (series_name.clone(), item.name.clone().into()),
            ItemType::Series | ItemType::MusicAlbum => (item.name.clone(), None),
            ItemType::Playlist | ItemType::Folder => (item.name.clone(), None),
            ItemType::Music { album_id: _, album } => (album.clone(), item.name.clone().into()),
            ItemType::Unknown => return Ok(None),
            ItemType::CollectionFolder => return Ok(None),
        };
        let image = select_images(&item)
            .map(|(image_type, tag)| {
                JellyfinImage::new(
                    item.id.clone(),
                    tag.to_string(),
                    image_type,
                    jellyfin.clone(),
                    db.clone(),
                    availabe.clone(),
                    cache.clone(),
                    picker.clone(),
                    stats.clone(),
                )
            })
            .next();
        let watch_status = if let Some(user_data) = item.user_data.as_ref() {
            if let Some(num @ 1..) = user_data.unplayed_item_count {
                Some(format!("{num}").into())
            } else if user_data.played {
                Some("✓".into())
            } else {
                None
            }
        } else {
            None
        };
        Ok(Some(Self::new(
            image,
            title,
            subtitle,
            EntryInner::Item(item),
            watch_status,
        )))
    }

    pub fn from_user_view(
        item: UserView,
        jellyfin: &JellyfinClient,
        db: &Arc<tokio::sync::Mutex<SqliteConnection>>,
        cache: &ImageProtocolCache,
        availabe: &ImagesAvailable,
        picker: &Arc<Picker>,
        stats: &Stats,
    ) -> Result<Self> {
        let title = item.name.clone();
        let image = item
            .image_tags
            .iter()
            .flat_map(|map| map.iter())
            .next()
            .map(|(image_type, tag)| {
                JellyfinImage::new(
                    item.id.clone(),
                    tag.clone(),
                    *image_type,
                    jellyfin.clone(),
                    db.clone(),
                    availabe.clone(),
                    cache.clone(),
                    picker.clone(),
                    stats.clone(),
                )
            });
        Ok(Self::new(image, title, None, EntryInner::View(item), None))
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EntryInner {
    Item(MediaItem),
    View(UserView),
}
