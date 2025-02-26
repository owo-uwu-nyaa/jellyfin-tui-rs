use std::{cmp::min, iter::repeat_n};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, BorderType, Padding, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_image::{picker::Picker, FontSize};
use tracing::{instrument, trace};

use crate::{
    entry::{entry_height, Entry, ENTRY_WIDTH},
    image::ImagesAvailable,
    NextScreen,
};

pub struct EntryList {
    entries: Vec<Entry>,
    current: usize,
    title: String,
}

impl EntryList {
    pub fn new(entries: Vec<Entry>, title: String) -> Self {
        Self {
            entries,
            current: 0,
            title,
        }
    }

    #[instrument(skip_all)]
    pub fn prefetch(&mut self, availabe: &ImagesAvailable, area: Rect) {
        let visible = self.visible(area.width);
        for entry in self.entries.iter_mut().take(visible) {
            entry.prefetch(availabe);
        }
    }

    #[instrument(skip_all)]
    pub fn render_list(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        availabe: &ImagesAvailable,
        picker: &Picker,
        active: bool,
    ) {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        outer.render(area, buf);
        let visible = self.visible(area.width);
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
            let border_type = if active && i == current {
                BorderType::Double
            } else {
                BorderType::Rounded
            };
            entries[i].render_entry(areas[i], buf, availabe, picker, border_type)
        }
        if visible < entries.len() {
            entries[visible].prefetch(availabe);
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

    pub fn get(mut self) -> NextScreen {
        self.entries.swap_remove(self.current).get()
    }
}

pub fn entry_list_height(font: FontSize) -> u16 {
    entry_height(font) + 4
}
