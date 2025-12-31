use std::{pin::Pin, time::Duration};

use color_eyre::Result;
use jellyhaj_core::{context::TuiContext, keybinds::LoggerCommand, state::Navigation};
use keybinds::{KeybindEvent, KeybindEventStream, StreamExt};
use ratatui::{
    style::{Color, Style},
    widgets::{Block, Padding, Widget},
};
use ratatui_fallible_widget::TermExt;
use tokio::select;
use tui_logger::{TuiLoggerLevelOutput, TuiWidgetEvent};

struct LogView {
    state: tui_logger::TuiWidgetState,
}

impl Widget for &LogView {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered()
            .title("Log Messages")
            .padding(Padding::uniform(1));
        tui_logger::TuiLoggerSmartWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Green))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Cyan))
            .output_separator(':')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(false)
            .output_line(false)
            .state(&self.state)
            .render(block.inner(area), buf);
        block.render(area, buf);
    }
}

pub async fn show_tui(cx: Pin<&mut TuiContext>) -> Result<Navigation> {
    let cx = cx.project();
    let state = tui_logger::TuiWidgetState::new();
    let mut widget = LogView { state };
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut widget,
        cx.config.keybinds.logger.clone(),
        &cx.config.help_prefixes,
    );
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        cx.term.draw_fallible(&mut events)?;
        let command = select! {
        biased;
        event = events.next() => {
            match event {
                None => break  Ok(Navigation::Exit),
                Some(Err(e)) => break  Err(e),
                Some(Ok(KeybindEvent::Render)) => continue,
                Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                Some(Ok(KeybindEvent::Command(c))) => match c{
                    LoggerCommand::Space => TuiWidgetEvent::SpaceKey,
                    LoggerCommand::TargetUp => TuiWidgetEvent::UpKey,
                    LoggerCommand::TargetDown => TuiWidgetEvent::DownKey,
                    LoggerCommand::Left => TuiWidgetEvent::LeftKey,
                    LoggerCommand::Right => TuiWidgetEvent::RightKey,
                    LoggerCommand::Plus => TuiWidgetEvent::PlusKey,
                    LoggerCommand::Minus => TuiWidgetEvent::MinusKey,
                    LoggerCommand::Hide => TuiWidgetEvent::HideKey,
                    LoggerCommand::Focus => TuiWidgetEvent::FocusKey,
                    LoggerCommand::MessagesUp => TuiWidgetEvent::PrevPageKey,
                    LoggerCommand::MessagesDown => TuiWidgetEvent::NextPageKey,
                    LoggerCommand::Escape => TuiWidgetEvent::EscapeKey,
                    LoggerCommand::Quit => break Ok(Navigation::PopContext),
                }
            }
        }
        _ = interval.tick() => {
            continue
        }
        };
        events.get_inner().state.transition(command);
    }
}
