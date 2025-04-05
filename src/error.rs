use std::{cmp::min, pin::Pin};

use ansi_to_tui::IntoText;
use color_eyre::eyre::{Context, Report, Result};
use futures_util::StreamExt;
use keybinds::{Command, KeybindEvent, KeybindEventStream};
use ratatui::{
    layout::Margin,
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{state::Navigation, TuiContext};

#[derive(Debug, Clone, Copy, Command)]
pub enum ErrorCommand {
    Quit,
    Kill,
    Up,
    Down,
    Left,
    Right,
}

pub async fn display_error(cx: Pin<&mut TuiContext>, e: Report) -> Result<Navigation> {
    tracing::error!("Error encountered: {e:?}");
    let text = format!("{e:?}");
    let text = text
        .to_text()
        .context("handling color eyre error message")?;
    let width = text.width();
    let height = text.height();
    let mut pos_x = 0;
    let mut pos_y = 0;
    let mut text = Paragraph::new(text).block(
        Block::bordered()
            .title("Error encountered")
            .padding(Padding::uniform(1)),
    );
    let cx = cx.project();
    let mut events = KeybindEventStream::new(cx.events, cx.config.keybinds.error.clone());
    loop {
        let mut scroll_x = 0;
        let mut scroll_y = 0;
        cx.term
            .draw(|frame| {
                let area = events.inner(frame.area());
                scroll_x = width.saturating_sub(area.width as usize);
                scroll_y = height.saturating_sub(area.height as usize);
                pos_x = min(scroll_x.saturating_sub(1), pos_x);
                pos_y = min(scroll_y.saturating_sub(1), pos_y);
                text = std::mem::take(&mut text).scroll((pos_y as u16, pos_x as u16));
                frame.render_widget(&text, area);
                if scroll_y > 0 {
                    frame.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight),
                        area.inner(Margin::new(0, 2)),
                        &mut ScrollbarState::new(scroll_y).position(pos_y),
                    )
                }
                if scroll_x > 0 {
                    frame.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
                        area.inner(Margin::new(2, 0)),
                        &mut ScrollbarState::new(scroll_x).position(pos_x),
                    )
                }

                frame.render_widget(&mut events, frame.area());
            })
            .context("rendering error")?;
        match events.next().await {
            Some(Ok(KeybindEvent::Render)) => continue,
            Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
            Some(Ok(KeybindEvent::Command(command))) => match command {
                ErrorCommand::Quit => break Ok(Navigation::PopContext),
                ErrorCommand::Kill => break Ok(Navigation::Exit),
                ErrorCommand::Up => {
                    pos_y = pos_y.saturating_sub(1);
                }
                ErrorCommand::Down => {
                    pos_y = min(scroll_y.saturating_sub(1), pos_y + 1);
                }
                ErrorCommand::Left => {
                    pos_x = pos_x.saturating_sub(1);
                }
                ErrorCommand::Right => pos_x = min(scroll_x.saturating_sub(1), pos_x + 1),
            },
            Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
            None => break Ok(Navigation::Exit),
        }
    }
}
