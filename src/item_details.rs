use std::{cmp::min, pin::Pin};

use crate::{
    entry::{entry_height, Entry, ENTRY_WIDTH},
    fetch::{fetch_child_of_type, fetch_screen},
    image::ImagesAvailable,
    state::{Navigation, NextScreen, ToNavigation},
    TuiContext,
};
use color_eyre::{eyre::Context, Result};
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use keybinds::{Command, KeybindEvent, KeybindEventStream};
use ratatui::{
    layout::{Constraint, Layout, Margin},
    text::Text,
    widgets::{Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarState},
};

#[derive(Debug, Clone, Copy, Command)]
pub enum EpisodeCommand {
    Quit,
    Up,
    Down,
    Play,
}

pub async fn display_fetch_episode(cx: Pin<&mut TuiContext>, parent: &str) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        "fetching episode",
        async {
            Ok(fetch_child_of_type(jellyfin, "Episode, Movie", parent)
                .await
                .context("fetching episode")
                .map(|item| Navigation::Replace(NextScreen::ItemDetails(item)))
                .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}

//also works with movies
pub async fn display_item_details(cx: Pin<&mut TuiContext>, item: MediaItem) -> Result<Navigation> {
    let mut entry = Entry::from_media_item(item.clone(), &cx);
    let images_available = ImagesAvailable::new();
    let cx = cx.project();
    let mut events = KeybindEventStream::new(cx.events, cx.config.keybinds.item_details.clone());
    let block = Block::bordered()
        .title(item.name.as_str())
        .padding(ratatui::widgets::Padding::uniform(1));
    let mut width = None;
    let mut scrollbar_state = ScrollbarState::new(0);
    let mut scrollbar_pos = 0;
    let mut scrollbar_len = 0;
    let mut descr = None;
    loop {
        cx.term
            .draw(|frame| {
                let height = entry_height(cx.image_picker.font_size());
                let main = block.inner(frame.area());
                let [entry_area, descripton_area] =
                    Layout::vertical([Constraint::Length(height), Constraint::Min(1)])
                        .spacing(1)
                        .areas(main);
                let [entry_area] =
                    Layout::horizontal([Constraint::Length(ENTRY_WIDTH)]).areas(entry_area);
                entry.render(
                    entry_area,
                    frame.buffer_mut(),
                    &images_available,
                    cx.image_picker,
                    BorderType::Plain,
                );
                let w = descripton_area.width.saturating_sub(4);
                if width != Some(w) {
                    width = Some(w);
                    if let Some(d) = &item.overview {
                        let lines = textwrap::wrap(d, w as usize);
                        scrollbar_state = scrollbar_state.content_length(lines.len());
                        scrollbar_len = lines.len() as u16;
                        scrollbar_pos = min(scrollbar_pos, scrollbar_len - 1);
                        descr = Some(
                            Paragraph::new(Text::from_iter(lines.into_iter())).block(
                                Block::bordered()
                                    .title("Overview")
                                    .padding(Padding::uniform(1)),
                            ),
                        );
                    }
                }
                if let Some(descr) = &mut descr {
                    frame.render_widget(descr.clone().scroll((scrollbar_pos, 0)), descripton_area);
                    frame.render_stateful_widget(
                        Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
                        descripton_area.inner(Margin {
                            horizontal: 0,
                            vertical: 2,
                        }),
                        &mut scrollbar_state,
                    );
                }
            })
            .context("drawing episode/movie")?;
        let cmd = tokio::select! {
            _ = images_available.wait_available() => {continue          }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(cmd))) => cmd,
                    Some(Ok(KeybindEvent::Render)) => continue ,
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Err(e)) => break  Err(e).context("getting key events from terminal"),
                    None => break  Ok(Navigation::PopContext)
                }
            }
        };
        match cmd {
            EpisodeCommand::Quit => break Ok(Navigation::PopContext),
            EpisodeCommand::Up => {
                scrollbar_pos = min(scrollbar_pos + 1, scrollbar_len - 1);
            }
            EpisodeCommand::Down => {
                scrollbar_pos = scrollbar_pos.saturating_sub(1);
            }
            EpisodeCommand::Play => {
                let next = NextScreen::LoadPlayItem(crate::entry::media_item::play(&item));
                break Ok(Navigation::Push {
                    current: NextScreen::ItemDetails(item),
                    next,
                });
            }
        }
    }
}
