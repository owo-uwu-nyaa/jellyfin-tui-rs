pub mod fetch_items;

use std::{borrow::Cow, pin::Pin};

use color_eyre::eyre::{Context, Result, eyre};
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use jellyfin_tui_core::{
    context::TuiContext,
    keybinds::MpvCommand,
    state::{Navigation, NextScreen},
};
use keybinds::{KeybindEvent, KeybindEventStream};
use player_core::{Command, PlayerHandle, PlayerState};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Padding, Paragraph, Widget},
};
use ratatui_fallible_widget::{FallibleWidget, TermExt};
use tokio::{select, sync::watch};
use tracing::{info, instrument};

struct MinimizeGuard {
    handle: PlayerHandle,
}

//TODO: overwrite q keybinding
//TODO: fix out of bounds index access

impl Drop for MinimizeGuard {
    fn drop(&mut self) {
        self.handle.send(Command::Stop);
    }
}

#[instrument(skip_all)]
pub async fn play(
    cx: Pin<&mut TuiContext>,
    items: Vec<MediaItem>,
    index: usize,
) -> Result<Navigation> {
    if items.is_empty() {
        return Ok(Navigation::Replace(NextScreen::Error(eyre!(
            "Unable to play, item is empty"
        ))));
    }
    let cx = cx.project();
    cx.mpv_handle.send(Command::Minimized(false));
    cx.mpv_handle.send(Command::Fullscreen(true));
    cx.mpv_handle.send(Command::ReplacePlaylist {
        items,

        first: index,
    });
    let minimize = MinimizeGuard {
        handle: cx.mpv_handle.clone(),
    };
    let mut widget = PlayerWidget {
        state: cx.mpv_handle.state().clone(),
    };
    let mut events =
        KeybindEventStream::new(cx.events, &mut widget, cx.config.keybinds.play_mpv.clone());
    let mut idle = cx.mpv_handle.state().borrow().idle;
    loop {
        cx.term.clear()?;
        cx.term.draw_fallible(&mut events)?;
        select! {
            res = cx.mpv_handle.state_mut().changed()=> {
                if res.is_err(){
                    info!("mpv sender is closed, exiting");
                    break;
                }else if idle != cx.mpv_handle.state().borrow().idle {
                    if !idle {
                        info!("mpv is idle, exiting");
                        break;
                    }else {
                        idle = false
                    }
                }
            }
            event = events.next() => {
                match event {
                    Some(Ok(KeybindEvent::Command(MpvCommand::Quit)))
                     => {break;}
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Ok(KeybindEvent::Render)) => {},
                    Some(Err(e)) => return Err(e).context("getting key events from terminal"),
                    None => {return Ok(Navigation::Exit);}
                }
            }
        }
    }
    //some ffmpeg stuff still writes to stdout
    cx.term.clear()?;
    drop(minimize);
    Ok(Navigation::PopContext)
}

struct PlayerWidget {
    state: watch::Receiver<PlayerState>,
}

impl FallibleWidget for PlayerWidget {
    fn render_fallible(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> Result<()> {
        let state = self.state.borrow();
        let block_area = area;
        let block = Block::bordered()
            .title("Now playing")
            .padding(Padding::uniform(1));
        let area = block.inner(block_area);
        if let Some(index) = state.current {
            let media_item = &state.playlist[index].item;
            match &media_item.item_type {
                jellyfin::items::ItemType::Movie => {
                    Paragraph::new(media_item.name.clone())
                        .centered()
                        .render(area, buf);
                }
                jellyfin::items::ItemType::Episode {
                    season_id: _,
                    season_name: None,
                    series_id: _,
                    series_name,
                } => {
                    let [series, episode] =
                        Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                            .vertical_margin(3)
                            .areas(area);
                    let mut series_str = Cow::from(series_name.as_str());
                    if media_item.episode_index.is_some() || media_item.season_index.is_some() {
                        series_str.to_mut().push(' ');
                        if let Some(season) = media_item.season_index {
                            series_str.to_mut().push('S');
                            series_str.to_mut().push_str(&season.to_string());
                        }
                        if let Some(episode) = media_item.episode_index {
                            series_str.to_mut().push('E');
                            series_str.to_mut().push_str(&episode.to_string());
                        }
                    }

                    Paragraph::new(series_str).centered().render(series, buf);
                    Paragraph::new(media_item.name.clone())
                        .centered()
                        .render(episode, buf);
                }
                jellyfin::items::ItemType::Episode {
                    season_id: _,
                    season_name: Some(season_name),
                    series_id: _,
                    series_name,
                } => {
                    let [series, season, episode] = Layout::vertical([
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                        Constraint::Fill(1),
                    ])
                    .vertical_margin(3)
                    .areas(area);
                    let mut series_str = Cow::from(series_name.as_str());
                    if media_item.episode_index.is_some() || media_item.season_index.is_some() {
                        series_str.to_mut().push(' ');
                        if let Some(season) = media_item.season_index {
                            series_str.to_mut().push('S');
                            series_str.to_mut().push_str(&season.to_string());
                        }
                        if let Some(episode) = media_item.episode_index {
                            series_str.to_mut().push('E');
                            series_str.to_mut().push_str(&episode.to_string());
                        }
                    }
                    Paragraph::new(series_str).centered().render(series, buf);
                    Paragraph::new(season_name.clone())
                        .centered()
                        .render(season, buf);
                    Paragraph::new(media_item.name.clone())
                        .centered()
                        .render(episode, buf);
                }
                _ => {
                    panic!("unexpected media item type: {media_item:#?}");
                }
            }
        } else {
            Paragraph::new("Nowthing is currently playing").render(area, buf);
        }
        block.render(block_area, buf);
        Ok(())
    }
}
