use std::{cmp::min, iter::repeat_n, sync::Arc};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget, Wrap},
};
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::picker::Picker;
use tracing::{instrument, trace};

use crate::{
    entry::{ENTRY_WIDTH, Entry},
    list::{EntryList, entry_list_height},
};

#[derive(Debug)]
pub struct EntryScreen {
    entries: Vec<EntryList>,
    current: usize,
    title: String,
    picker: Arc<Picker>,
}

impl FallibleWidget for EntryScreen {
    #[instrument(skip_all, name = "render_screen")]
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
        let entry_height = entry_list_height(self.picker.font_size());
        let visible = self.visible(area.height, entry_height);
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
            entries = &mut entries[..visible];
        }
        let areas = Layout::vertical(repeat_n(Constraint::Length(entry_height), visible))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for i in 0..areas.len() {
            let entry = &mut entries[i];
            entry.active = i == current;
            entry.render_fallible(areas[i], buf)?
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
}

impl EntryScreen {
    pub fn new(entries: Vec<EntryList>, title: String, picker: Arc<Picker>) -> Self {
        Self {
            entries,
            current: 0,
            title,
            picker,
        }
    }

    #[instrument(skip_all)]
    pub fn up(&mut self) {
        self.current = self.current.saturating_sub(1);
        trace!("current: {}, length: {}", self.current, self.entries.len());
    }

    #[instrument(skip_all)]
    pub fn down(&mut self) {
        let new = self.current + 1;
        if self.entries.len() > new {
            self.current = new;
        }
        trace!("current: {}, length: {}", self.current, self.entries.len());
    }

    #[instrument(skip_all)]
    pub fn left(&mut self) {
        self.entries[self.current].left();
    }

    #[instrument(skip_all)]
    pub fn right(&mut self) {
        self.entries[self.current].right();
    }

    pub fn get(&self) -> Option<&Entry> {
        if self.entries.is_empty() {
            None
        } else {
            self.entries[self.current].get()
        }
    }

    fn visible(&self, height: u16, entry_height: u16) -> usize {
        min(((height - 5) / (entry_height)).into(), self.entries.len())
    }
}
