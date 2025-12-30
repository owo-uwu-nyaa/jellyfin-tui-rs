use std::{cmp::max, pin::Pin, sync::atomic::Ordering::Relaxed, time::Duration};

use jellyfin_tui_core::{context::TuiContext, keybinds::StatsCommand, state::Navigation};
use keybinds::{KeybindEvent, KeybindEventStream, StreamExt};
use ratatui::{
    layout::Constraint,
    symbols::merge::MergeStrategy,
    text::Text,
    widgets::{Block, Padding, Widget},
};
use ratatui_fallible_widget::TermExt;
use stats_data::Stats;
use tokio::select;

struct StatsWidget {
    stats: Stats,
}

struct BorderedTable<'r> {
    rows: &'r [&'r [&'r str]],
    col_widths: &'r [u16],
}

impl<'r> BorderedTable<'r> {
    fn new(rows: &'r [&'r [&'r str]], col_widths: &'r [u16]) -> Self {
        Self { rows, col_widths }
    }
    fn width(&self) -> u16 {
        if self.col_widths.is_empty() {
            0
        } else {
            let col_widths = self.col_widths.iter().copied().fold(0, u16::strict_add);
            let col_seps = u16::try_from(self.col_widths.len() - 1)
                .expect("overflow converting number of colns to u16")
                .strict_mul(3);
            col_widths.strict_add(col_seps).strict_add(4)
        }
    }
    fn height(&self) -> u16 {
        if self.col_widths.is_empty() {
            0
        } else {
            u16::try_from(self.rows.len())
                .expect("overflow converting row number to u16")
                .strict_mul(2)
                .strict_add(1)
        }
    }
}

impl Widget for &BorderedTable<'_> {
    fn render(self, mut area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if area.width < self.width() || area.height < self.height() {
            let size = format!("needs at least {}x{}", self.width(), self.height());
            Text::from_iter(["Terminal size not large enough", &size]).render(area, buf);
        } else {
            let block = Block::bordered()
                .padding(Padding::horizontal(1))
                .merge_borders(MergeStrategy::Exact);
            for row in self.rows {
                let mut row_area = area;
                row_area.height = 3;
                area.y = area.y.strict_add(2);
                area.height = area.height.strict_sub(2);
                assert_eq!(
                    row.len(),
                    self.col_widths.len(),
                    "mismatch in the number of colums"
                );
                for (cell, width) in row.iter().zip(self.col_widths) {
                    let width = width.strict_add(4);
                    let mut cell_area = row_area;
                    cell_area.width = width;
                    row_area.x = row_area.x.strict_add(width - 1);
                    row_area.width = row_area.width.strict_sub(width - 1);
                    cell.render(block.inner(cell_area), buf);
                    (&block).render(cell_area, buf);
                }
            }
        }
    }
}

impl Widget for &StatsWidget {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered().title("Program stats");
        let image_fetches = self.stats.image_fetches.load(Relaxed).to_string();
        let image_fetchers = ["Image fetches", &image_fetches];
        let db_image_cache_hits = self.stats.db_image_cache_hits.load(Relaxed).to_string();
        let db_image_cache_hits = ["DB image cache hits", &db_image_cache_hits];
        let memory_image_cache_hits = self.stats.memory_image_cache_hits.load(Relaxed).to_string();
        let memory_image_cache_hits = ["In memory image cache hits", &memory_image_cache_hits];
        let rows: [&[_]; _] = [
            &image_fetchers,
            &db_image_cache_hits,
            &memory_image_cache_hits,
        ];
        let (col1, col2) = rows.iter().fold((0, 0), |(col1, col2), v| {
            (max(col1, v[0].len()), max(col2, v[1].len()))
        });
        let cols = [col1 as u16, col2 as u16];
        let table = BorderedTable::new(&rows, &cols);
        let table_area = block.inner(area).centered(
            Constraint::Length(table.width()),
            Constraint::Length(table.height()),
        );
        table.render(table_area, buf);
        block.render(area, buf);
    }
}

pub async fn show_stats(cx: Pin<&mut TuiContext>) -> color_eyre::Result<Navigation> {
    let cx = cx.project();
    let mut widget = StatsWidget {
        stats: cx.stats.clone(),
    };
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut widget,
        cx.config.keybinds.stats.clone(),
        &cx.config.help_prefixes,
    );
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        cx.term.draw_fallible(&mut events)?;
        select! {
            biased;
            event = events.next() => {
                match event{
                    Some(Ok(KeybindEvent::Render)) => continue,
                    Some(Ok(KeybindEvent::Command(StatsCommand::Quit))) => {
                        break Ok(Navigation::PopContext);
                    }
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Err(e)) => break Err(e),
                    None => break Ok(Navigation::Exit),
                }
            }
            _ = interval.tick() => {
                continue
            }
        }
    }
}
