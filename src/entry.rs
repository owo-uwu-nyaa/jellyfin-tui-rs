
use ratatui::{
    layout::Rect,
    widgets::{Block, BorderType, Widget},
};
use ratatui_image::{picker::Picker, FontSize};

use crate::{image::{ImagesAvailable, JellyfinImage, JellyfinImageState}, NextScreen, Result};

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
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        availabe: &ImagesAvailable,
        picker: &Picker,
        border_type: BorderType,
    ) -> Result<()> {
        let mut outer = Block::bordered()
            .border_type(border_type)
            .title_top(self.title.as_str());
        if let Some(subtitle) = &self.subtitle {
            outer = outer.title_bottom(subtitle.as_str());
        }
        let inner = outer.inner(area);
        outer.render(area, buf);
        if let Some(state) = &mut self.image {
            JellyfinImage::default().render(inner, buf, state, availabe, picker)?;
        }
        Ok(())
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

    pub fn get(self) -> NextScreen {
        self.action
    }
}
