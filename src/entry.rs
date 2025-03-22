use std::borrow::Cow;

use jellyfin::{
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use ratatui::{
    layout::Rect,
    style::Color,
    text::Span,
    widgets::{Block, BorderType, Paragraph, Widget},
};
use ratatui_image::{picker::Picker, FontSize};
use tracing::instrument;

use crate::{
    image::{ImagesAvailable, JellyfinImage, JellyfinImageState},
    state::NextScreen,
    TuiContext,
};

pub struct Entry {
    image: Option<JellyfinImageState>,
    title: String,
    subtitle: Option<String>,
    action: NextScreen,
    watch_status: Option<Cow<'static, str>>,
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
    #[instrument(skip_all)]
    pub fn render_entry(
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
            JellyfinImage::default().render_image(inner, buf, state, availabe, picker);
        }
    }

    #[instrument(skip_all)]
    pub fn prefetch(&mut self, availabe: &ImagesAvailable) {
        if let Some(image) = self.image.as_mut() {
            image.prefetch(availabe);
        }
    }

    pub fn new(
        image: Option<JellyfinImageState>,
        title: String,
        subtitle: Option<String>,
        action: NextScreen,
        watch_status: Option<Cow<'static, str>>,
    ) -> Self {
        Self {
            image,
            title,
            subtitle,
            action,
            watch_status,
        }
    }

    pub fn from_media_item(item: MediaItem, context: &TuiContext) -> Self {
        let (title, subtitle) = match &item.item_type {
            ItemType::Movie { container: _ } => (item.name.clone(), None),
            ItemType::Episode {
                container: _,
                season_id: _,
                season_name: _,
                series_id: _,
                series_name,
                seasion_index: _,
                episode_index: _,
            } => (series_name.clone(), item.name.clone().into()),
            ItemType::Season {
                series_id: _,
                series_name,
            } => (series_name.clone(), item.name.clone().into()),
            ItemType::Series => (item.name.clone(), None),
            ItemType::Playlist => (item.name.clone(), None),
        };
        let image = item
            .image_tags
            .iter()
            .flat_map(|map| map.iter())
            .next()
            .map(|(image_type, tag)| {
                JellyfinImageState::new(
                    &context.jellyfin,
                    context.cache.clone(),
                    tag.clone(),
                    item.id.clone(),
                    *image_type,
                    context.image_cache.clone(),
                )
            });
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
        Self::new(
            image,
            title,
            subtitle,
            NextScreen::LoadPlayItem(item),
            watch_status,
        )
    }

    pub fn from_user_view(item: UserView, context: &TuiContext) -> Self {
        let title = item.name.clone();
        let image = item
            .image_tags
            .iter()
            .flat_map(|map| map.iter())
            .next()
            .map(|(image_type, tag)| {
                JellyfinImageState::new(
                    &context.jellyfin,
                    context.cache.clone(),
                    tag.clone(),
                    item.id.clone(),
                    *image_type,
                    context.image_cache.clone(),
                )
            });
        Self::new(image, title, None, NextScreen::LoadUserView(item), None)
    }

    pub fn get_action(self) -> NextScreen {
        self.action
    }
}
