use std::{cmp::min, iter::repeat_n};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, BorderType, Padding, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_image::{picker::Picker, FontSize};

use crate::{
    entry::{entry_height, Entry, ENTRY_WIDTH},
    image::ImagesAvailable,
    NextScreen, Result,
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
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        availabe: &ImagesAvailable,
        picker: &Picker,
    ) -> Result<()> {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        outer.render(area, buf);
        let visible = self.visible(area.width);
        let mut entries = self.entries.as_mut_slice();
        let mut current = self.current;
        if visible < entries.len() {
            let visible_back = visible / 2;
            if current > visible_back {
                let offset = current - visible_back;
                current -= offset;
                entries = &mut entries[offset..];
            }
            entries = &mut entries[..visible];
        }
        let areas = Layout::horizontal(repeat_n(Constraint::Length(ENTRY_WIDTH), visible))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for i in 0..areas.len() {
            let border_type = if i == current {
                BorderType::Thick
            } else {
                BorderType::Rounded
            };
            entries[i].render(areas[i], buf, availabe, picker, border_type)?
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

    fn visible(&self, width: u16) -> usize {
        let max_visible: u16 = (width - 5) / (ENTRY_WIDTH + 1);
        min(max_visible.into(), self.entries.len())
    }

    pub fn left(&mut self) {
        self.current = self.current.saturating_sub(1)
    }

    pub fn right(&mut self) {
        let new = self.current + 1;
        if self.entries.len() < new {
            self.current = new;
        }
    }

    pub fn get(mut self) -> NextScreen {
        self.entries.swap_remove(self.current).get()
    }
}

pub fn entry_list_height(font: FontSize) -> u16 {
    entry_height(font) + 4
}
