use std::{cmp::min, iter::repeat_n};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{
        Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget,
        Wrap,
    },
};
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::FontSize;
use tracing::{instrument, trace};

use crate::{
    entry::{ENTRY_WIDTH, Entry, entry_height},
};

#[derive(Debug)]
pub struct EntryList {
    entries: Vec<Entry>,
    current: usize,
    title: String,
    pub active: bool,
}

impl FallibleWidget for EntryList {
    #[instrument(skip_all, name = "render_list")]
    fn render_fallible(&mut self, area: Rect, buf: &mut ratatui::prelude::Buffer) -> color_eyre::Result<()> {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        outer.render(area, buf);
        let visible = self.visible(area.width);
        if visible == 0 && !self.entries.is_empty() {
            Paragraph::new("insufficient space")
                .wrap(Wrap { trim: true })
                .render(main, buf);
            return Ok(());
        }
        let mut entries = self.entries.as_mut_slice();
        let mut current = self.current;
        if visible < entries.len() {
            let position_in_visible = visible / 2;
            if current > position_in_visible {
                let offset = min(current - position_in_visible, entries.len() - visible);
                current -= offset;
                entries = &mut entries[offset..];
            }
        }
        let areas = Layout::horizontal(repeat_n(Constraint::Length(ENTRY_WIDTH), visible))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for i in 0..visible {
            let border_type = if self.active && i == current {
                BorderType::Double
            } else {
                BorderType::Rounded
            };
            let entry = &mut entries[i];
            entry.border_type = border_type;
            entry.render_fallible(areas[i], buf)?
        }
        if visible < self.entries.len() {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::HorizontalBottom).render(
                area,
                buf,
                &mut ScrollbarState::new(self.entries.len())
                    .position(self.current)
                    .viewport_content_length(ENTRY_WIDTH as usize + 1),
            );
        }
        Ok(())
    }
}

impl EntryList {
    pub fn new(entries: Vec<Entry>, title: String) -> Self {
        Self {
            entries,
            current: 0,
            title,
            active: false,
        }
    }

    fn visible(&self, width: u16) -> usize {
        let max_visible: u16 = (width - 5) / (ENTRY_WIDTH + 1);
        min(max_visible.into(), self.entries.len())
    }

    #[instrument(skip_all)]
    pub fn left(&mut self) {
        self.current = self.current.saturating_sub(1);
        trace!("current: {}, length: {}", self.current, self.entries.len());
    }

    #[instrument(skip_all)]
    pub fn right(&mut self) {
        let new = self.current + 1;
        if self.entries.len() > new {
            self.current = new;
        }
        trace!("current: {}, length: {}", self.current, self.entries.len());
    }

    pub fn get(&self) -> Option<&Entry> {
        if self.entries.is_empty() {
            None
        } else {
            Some(&self.entries[self.current])
        }
    }
}

pub fn entry_list_height(font: FontSize) -> u16 {
    entry_height(font) + 4
}
