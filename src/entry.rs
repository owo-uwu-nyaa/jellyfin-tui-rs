use jellyfin::{
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use ratatui::{
    layout::Rect,
    widgets::{Block, BorderType, Widget},
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
}

const IMAGE_WIDTH: u16 = 32;
fn image_height(font: FontSize) -> u16 {
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
    ) -> Self {
        Self {
            image,
            title,
            subtitle,
            action,
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
        Self::new(image, title, subtitle, NextScreen::LoadPlayItem(item))
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
        Self::new(image, title, None, NextScreen::LoadUserView(item))
    }

    pub fn get_action(self) -> NextScreen {
        self.action
    }
}
