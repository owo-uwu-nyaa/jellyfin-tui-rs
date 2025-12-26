use std::cmp::min;

use ansi_to_tui::IntoText;
use color_eyre::eyre::{Context, Report, Result};
use futures_util::StreamExt;
use jellyfin_tui_core::{
    keybinds::{ErrorCommand, Keybinds},
    state::Navigation,
};
use keybinds::{KeybindEvent, KeybindEventStream, KeybindEvents};
use ratatui::{
    DefaultTerminal,
    layout::Margin,
    widgets::{
        Block, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};
use ratatui_fallible_widget::{FallibleWidget, TermExt};

struct ErrorWidget {
    text: String,
    pos_x: usize,
    pos_y: usize,
    scroll_x: usize,
    scroll_y: usize,
}

impl FallibleWidget for ErrorWidget {
    fn render_fallible(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> Result<()> {
        let text = self
            .text
            .to_text()
            .context("handling color eyre error message")?;
        let width = text.width();
        let height = text.height();
        let mut text = Paragraph::new(text).block(
            Block::bordered()
                .title("Error encountered")
                .padding(Padding::uniform(1)),
        );
        self.scroll_x = width.saturating_sub(area.width as usize);
        self.scroll_y = height.saturating_sub(area.height as usize);
        self.pos_x = min(self.scroll_x.saturating_sub(1), self.pos_x);
        self.pos_y = min(self.scroll_y.saturating_sub(1), self.pos_y);
        text = std::mem::take(&mut text).scroll((self.pos_y as u16, self.pos_x as u16));
        text.render(area, buf);
        if self.scroll_y > 0 {
            Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
                area.inner(Margin::new(0, 2)),
                buf,
                &mut ScrollbarState::new(self.scroll_y).position(self.pos_y),
            );
        }
        if self.scroll_x > 0 {
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom).render(
                area.inner(Margin::new(2, 0)),
                buf,
                &mut ScrollbarState::new(self.scroll_x).position(self.pos_x),
            );
        }
        Ok(())
    }
}

pub trait ResultDisplayExt<T> {
    fn display_error(
        self,
        term: &mut DefaultTerminal,
        events: &mut KeybindEvents,
        keybinds: &Keybinds,
        help_prefixes: &[String],
    ) -> impl Future<Output = Option<T>>;
}

impl<T> ResultDisplayExt<T> for Result<T> {
    async fn display_error(
        self,
        term: &mut DefaultTerminal,
        events: &mut KeybindEvents,
        keybinds: &Keybinds,
        help_prefixes: &[String],
    ) -> Option<T> {
        match self {
            Err(e) => {
                if let Some(e) = display_error(term, events, keybinds, help_prefixes, e)
                    .await
                    .err()
                {
                    tracing::error!("Error displaying error: {e:?}");
                }
                None
            }
            Ok(v) => Some(v),
        }
    }
}

pub async fn display_error(
    term: &mut DefaultTerminal,
    events: &mut KeybindEvents,
    keybinds: &Keybinds,
    help_prefixes: &[String],
    e: Report,
) -> Result<Navigation> {
    tracing::error!("Error encountered: {e:?}");
    let mut widget = ErrorWidget {
        text: format!("{e:?}"),
        pos_x: 0,
        pos_y: 0,
        scroll_x: 0,
        scroll_y: 0,
    };
    let mut events =
        KeybindEventStream::new(events, &mut widget, keybinds.error.clone(), help_prefixes);
    loop {
        term.draw_fallible(&mut events)?;
        match events.next().await {
            Some(Ok(KeybindEvent::Render)) => continue,
            Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
            Some(Ok(KeybindEvent::Command(command))) => match command {
                ErrorCommand::Quit => break Ok(Navigation::PopContext),
                ErrorCommand::Kill => break Ok(Navigation::Exit),
                ErrorCommand::Up => {
                    events.get_inner().pos_y = events.get_inner().pos_y.saturating_sub(1);
                }
                ErrorCommand::Down => {
                    events.get_inner().pos_y = min(
                        events.get_inner().scroll_y.saturating_sub(1),
                        events.get_inner().pos_y + 1,
                    );
                }
                ErrorCommand::Left => {
                    events.get_inner().pos_x = events.get_inner().pos_x.saturating_sub(1);
                }
                ErrorCommand::Right => {
                    events.get_inner().pos_x = min(
                        events.get_inner().scroll_x.saturating_sub(1),
                        events.get_inner().pos_x + 1,
                    )
                }
            },
            Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
            None => break Ok(Navigation::Exit),
        }
    }
}
