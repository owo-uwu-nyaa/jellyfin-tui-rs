use std::task;

use ratatui::{
    layout::Rect,
    widgets::{Block, BorderType, Clear},
    Frame,
};
use ratatui_image::{picker::Picker, FontSize, Resize, StatefulImage};

use crate::{image::LoadImage, NextScreen, Result};

pub struct Entry {
    image: Option<LoadImage>,
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
        cx: &mut task::Context,
        frame: &mut Frame<'_>,
        area: Rect,
        picker: &Picker,
        resize: impl Fn() -> Resize,
        border_type: BorderType,
    ) -> Result<()> {
        let mut outer = Block::bordered()
            .border_type(border_type)
            .title_top(self.title.as_str());
        if let Some(subtitle) = &self.subtitle {
            outer = outer.title_bottom(subtitle.as_str());
        }
        frame.render_widget(&outer, area);
        let area = outer.inner(area);
        if let Some(image) = &mut self.image {
            if let Some(image) = image.poll(cx, picker, resize(), area)? {
                frame.render_stateful_widget(
                    StatefulImage::default().resize(resize()),
                    area,
                    image,
                );
            } else {
                frame.render_widget(Clear, area);
            }
        } else {
            frame.render_widget(Clear, area);
        }
        Ok(())
    }

    pub fn new(
        image: Option<LoadImage>,
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
