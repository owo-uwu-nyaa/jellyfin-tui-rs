use std::{cmp::min, iter::repeat_n};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, Padding, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_image::picker::Picker;

use super::list::{entry_list_height, EntryList};
use crate::{entry::ENTRY_WIDTH, image::ImagesAvailable, NextScreen, Result};

pub struct EntryScreen {
    entries: Vec<EntryList>,
    current: usize,
    title: String,
}

impl EntryScreen {
    pub fn new(entries: Vec<EntryList>, title: String) -> Self {
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
        let entry_height = entry_list_height(picker.font_size());
        let visible = self.visible(area.height, entry_height);
        let mut entries = self.entries.as_mut_slice();
        let current = self.current;
        if visible < entries.len() {
            let visible_back = visible / 2;
            if current > visible_back {
                let offset = current - visible_back;
                entries = &mut entries[offset..];
            }
            entries = &mut entries[..visible];
        }
        let areas = Layout::vertical(repeat_n(Constraint::Length(entry_height), visible))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for i in 0..areas.len() {
            entries[i].render(areas[i], buf, availabe, picker)?
        }
        if visible < self.entries.len() {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight).render(
                area,
                buf,
                &mut ScrollbarState::new(self.entries.len())
                    .position(self.current)
                    .viewport_content_length(ENTRY_WIDTH as usize + 1),
            );
        }
        Ok(())
    }

    pub fn up(&mut self) {
        self.current = self.current.saturating_sub(1)
    }

    pub fn down(&mut self) {
        let new = self.current + 1;
        if self.entries.len() < new {
            self.current = new;
        }
    }

    pub fn left(&mut self) {
        self.entries[self.current].left();
    }

    pub fn right(&mut self) {
        self.entries[self.current].right();
    }

    pub fn get(mut self) -> NextScreen {
        self.entries.swap_remove(self.current).get()
    }

    fn visible(&self, height: u16, entry_height: u16) -> usize {
        min(((height - 5) / (entry_height)).into(), self.entries.len())
    }
}
