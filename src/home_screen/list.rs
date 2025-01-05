use std::{cmp::min, iter::repeat_n, task};

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, BorderType, Padding, Scrollbar, ScrollbarState},
    Frame,
};
use ratatui_image::{picker::Picker, FontSize, Resize};

use crate::{
    entry::{entry_height, Entry, ENTRY_WIDTH},
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
        cx: &mut task::Context,
        frame: &mut Frame<'_>,
        area: Rect,
        picker: &Picker,
        resize: impl Fn() -> Resize,
    ) -> Result<()> {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        frame.render_widget(&outer, area);
        let main = outer.inner(area);
        let visible = self.visible(area.width);
        let mut entries = self.entries.as_mut_slice();
        let mut current = self.current;
        if visible < entries.len() {
            let visible_back = visible.div_floor(2);
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
            entries[i].render(cx, frame, areas[i], picker, &resize, border_type)?
        }
        if visible < self.entries.len() {
            frame.render_stateful_widget(
                Scrollbar::new(ratatui::widgets::ScrollbarOrientation::HorizontalBottom),
                area,
                &mut ScrollbarState::new(self.entries.len())
                    .position(self.current)
                    .viewport_content_length(ENTRY_WIDTH as usize + 1),
            );
        }
        Ok(())
    }

    fn visible(&self, width: u16) -> usize {
        min(
            (width - 5).div_floor(ENTRY_WIDTH + 1).into(),
            self.entries.len(),
        )
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
