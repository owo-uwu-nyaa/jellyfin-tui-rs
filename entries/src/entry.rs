use std::{borrow::Cow, fmt::Debug};

use jellyfin::{
    JellyfinClient,
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use ratatui::{
    layout::Rect,
    style::Color,
    text::Span,
    widgets::{Block, BorderType, Paragraph, Widget},
};
use ratatui_image::{FontSize, picker::Picker};
use sqlx::SqlitePool;
use tracing::instrument;

use crate::image::{
    JellyfinImage, available::ImagesAvailable, cache::ImageProtocolCache, state::JellyfinImageState,
};
use color_eyre::Result;

pub struct Entry {
    image: Option<JellyfinImageState>,
    title: String,
    subtitle: Option<String>,
    inner: EntryInner,
    watch_status: Option<Cow<'static, str>>,
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

impl Entry {
    pub fn inner(&self) -> &EntryInner {
        &self.inner
    }

    #[instrument(skip_all, name = "render_entry")]
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        availabe: &ImagesAvailable,
        picker: &Picker,
        border_type: BorderType,
    ) {
        let mut outer = Block::bordered()
            .border_type(border_type)
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
        if let Some(state) = &mut self.image {
            JellyfinImage::default().render(inner, buf, state, availabe, picker);
        }
    }

    #[instrument(skip_all, name = "prefetch_entry")]
    pub fn prefetch(&mut self, availabe: &ImagesAvailable) {
        if let Some(image) = self.image.as_mut() {
            image.prefetch(availabe);
        }
    }

    pub fn new(
        image: Option<JellyfinImageState>,
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
        }
    }

    pub fn from_media_item(
        item: MediaItem,
        jellyfin: &JellyfinClient,
        db: &SqlitePool,
        cache: &ImageProtocolCache,
    ) -> Result<Self> {
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
            ItemType::Series => (item.name.clone(), None),
            ItemType::Playlist | ItemType::Folder => (item.name.clone(), None),
        };
        let image = item
            .image_tags
            .iter()
            .flat_map(|map| map.iter())
            .next()
            .map(|(image_type, tag)| {
                JellyfinImageState::new(
                    jellyfin,
                    db.to_owned(),
                    tag.clone(),
                    item.id.clone(),
                    *image_type,
                    cache.to_owned(),
                )
            })
            .transpose()?;
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
        Ok(Self::new(
            image,
            title,
            subtitle,
            EntryInner::Item(item),
            watch_status,
        ))
    }

    pub fn from_user_view(
        item: UserView,
        jellyfin: &JellyfinClient,
        db: &SqlitePool,
        cache: &ImageProtocolCache,
    ) -> Result<Self> {
        let title = item.name.clone();
        let image = item
            .image_tags
            .iter()
            .flat_map(|map| map.iter())
            .next()
            .map(|(image_type, tag)| {
                JellyfinImageState::new(
                    jellyfin,
                    db.to_owned(),
                    tag.clone(),
                    item.id.clone(),
                    *image_type,
                    cache.to_owned(),
                )
            })
            .transpose()?;
        Ok(Self::new(image, title, None, EntryInner::View(item), None))
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EntryInner {
    Item(MediaItem),
    View(UserView),
}
