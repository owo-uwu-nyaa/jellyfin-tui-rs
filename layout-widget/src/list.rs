use std::{cmp::min, iter::repeat_n};

use config::Config;
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect, Size},
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget, Wrap},
};
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::FontSize;
use tracing::{instrument, trace};

use crate::Item;

#[derive(Debug)]
pub struct List<T: Item> {
    items: Vec<T>,
    current: usize,
    title: String,
    pub active: bool,
    item_size: Size,
}

impl<T: Item> FallibleWidget for List<T> {
    #[instrument(skip_all, name = "render_list")]
    fn render_fallible(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> color_eyre::Result<()> {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        outer.render(area, buf);
        let visible = self.visible(area.width);
        if visible == 0 && !self.items.is_empty() {
            Paragraph::new("insufficient space")
                .wrap(Wrap { trim: true })
                .render(main, buf);
            return Ok(());
        }
        let mut items = self.items.as_mut_slice();
        let mut current = self.current;
        if visible < items.len() {
            let position_in_visible = visible / 2;
            if current > position_in_visible {
                let offset = min(current - position_in_visible, items.len() - visible);
                current -= offset;
                items = &mut items[offset..];
            }
        }
        let areas = Layout::horizontal(repeat_n(Constraint::Length(self.item_size.width), visible))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for i in 0..visible {
            let item = &mut items[i];
            item.set_active(self.active && i == current);
            item.render_fallible(areas[i], buf)?
        }
        if visible < self.items.len() {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::HorizontalBottom).render(
                area,
                buf,
                &mut ScrollbarState::new(self.items.len())
                    .position(self.current)
                    .viewport_content_length(self.item_size.width as usize + 1),
            );
        }
        Ok(())
    }
}

impl<T: Item> List<T> {
    pub fn new(entries: Vec<T>, title: String, config: &Config, font_size: FontSize) -> Self {
        Self {
            items: entries,
            current: 0,
            title,
            active: false,
            item_size: T::dimension(config, font_size),
        }
    }

    fn visible(&self, width: u16) -> usize {
        let max_visible: u16 = (width - 5) / (self.item_size.width + 1);
        min(max_visible.into(), self.items.len())
    }

    #[instrument(skip_all)]
    pub fn left(&mut self) {
        self.current = self.current.saturating_sub(1);
        trace!("current: {}, length: {}", self.current, self.items.len());
    }

    #[instrument(skip_all)]
    pub fn right(&mut self) {
        let new = self.current + 1;
        if self.items.len() > new {
            self.current = new;
        }
        trace!("current: {}, length: {}", self.current, self.items.len());
    }

    pub fn get(&self) -> Option<&T> {
        if self.items.is_empty() {
            None
        } else {
            Some(&self.items[self.current])
        }
    }
    pub fn entry_list_height(&self) -> u16 {
        self.item_size.height + 4
    }

    pub fn entry_list_height_static(config: &Config, font_size: FontSize) -> u16 {
        T::dimension(config, font_size).height + 4
    }
}
