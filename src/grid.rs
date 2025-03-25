use crate::{
    entry::{entry_height, Entry, ENTRY_WIDTH},
    image::ImagesAvailable,
};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{
        Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget,
        Wrap,
    },
};
use ratatui_image::picker::Picker;
use std::{cmp::min, iter::repeat_n};
use tracing::{debug, instrument, trace};

pub struct EntryGrid {
    entries: Vec<Entry>,
    current: usize,
    width: usize,
    title: String,
}

impl EntryGrid {
    pub fn new(entries: Vec<Entry>, title: String) -> Self {
        Self {
            entries,
            current: 0,
            width: 1,
            title,
        }
    }

    #[instrument(name = "render_grid", skip_all)]
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        availabe: &ImagesAvailable,
        picker: &Picker,
    ) {
        let outer = Block::bordered()
            .title_top(self.title.as_str())
            .padding(Padding::uniform(1));
        let main = outer.inner(area);
        outer.render(area, buf);
        self.width = ((main.width + 1) / (ENTRY_WIDTH + 1)).into();
        let entry_height = entry_height(picker.font_size());
        debug!("entry_height: {entry_height}");
        let height: usize = ((main.height + 1) / (entry_height + 1)).into();
        if height == 0 || self.width == 0 {
            Paragraph::new("insufficient space")
                .wrap(Wrap { trim: true })
                .render(main, buf);
            return;
        }
        debug!("height: {height}");
        let rows = self.entries.len().div_ceil(self.width);
        debug!("rows: {rows}");
        let row_index = self.current / self.width;
        let mut skip_rows = 0usize;
        if height < rows {
            let position = height / 2;
            if row_index > position {
                skip_rows = min(row_index - position, rows - height);
            }
        }
        debug!("skip_rows: {skip_rows}");
        let rendered_rows = min(height, rows);
        debug!("rendered_rows: {rendered_rows}");
        let row_areas = Layout::vertical(repeat_n(Constraint::Length(entry_height), height))
            .spacing(1)
            .flex(Flex::Start)
            .split(main);
        for row in skip_rows..skip_rows + rendered_rows {
            let area = row_areas[row - skip_rows];
            let areas = Layout::horizontal(repeat_n(Constraint::Length(ENTRY_WIDTH), self.width))
                .spacing(1)
                .flex(Flex::Start)
                .split(area);
            let first_entry = row * self.width;
            for entry in first_entry..first_entry + self.width {
                let area = areas[entry - first_entry];
                let border_type = if entry == self.current {
                    BorderType::Double
                } else {
                    BorderType::Rounded
                };
                if let Some(entry) = self.entries.get_mut(entry) {
                    entry.render(area, buf, availabe, picker, border_type);
                }
            }
        }
        for entry in self
            .entries
            .iter_mut()
            .skip(skip_rows + rendered_rows)
            .take(self.width)
        {
            entry.prefetch(availabe);
        }
        if height < rows {
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight).render(
                area,
                buf,
                &mut ScrollbarState::new(rows)
                    .position(row_index)
                    .viewport_content_length(entry_height as usize + 1),
            );
        }
    }
    #[instrument(skip_all)]
    pub fn up(&mut self) {
        self.current = self.current.saturating_sub(self.width);
        trace!("current: {}, length: {}", self.current, self.entries.len());
    }

    #[instrument(skip_all)]
    pub fn down(&mut self) {
        let new = self.current + self.width;
        if self.entries.len() > new {
            self.current = new;
        }
        trace!("current: {}, length: {}", self.current, self.entries.len());
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

    pub fn get(mut self) -> Entry {
        self.entries.swap_remove(self.current)
    }
}
