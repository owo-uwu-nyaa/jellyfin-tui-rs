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
use player_core::{PlayerHandle, PlayerState};
use player_jellyfin::player_jellyfin;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Padding, Paragraph, Widget},
};
use ratatui_fallible_widget::{FallibleWidget, TermExt};
use spawn::spawn;
use tokio::{select, sync::watch};
use tracing::{error_span, info, instrument};

#[instrument(skip_all)]
pub fn mk_player(
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
    let player = PlayerHandle::new(
        cx.jellyfin,
        &cx.config.hwdec,
        cx.config.mpv_profile,
        &cx.config.mpv_log_level,
        items,
        index,
    )?;
    spawn(
        player_jellyfin(player.new_ref(), cx.jellyfin.clone()),
        error_span!("player_jellyfin"),
    );
    Ok(Navigation::Replace(NextScreen::Play(player)))
}

#[instrument(skip_all)]
pub async fn play(cx: Pin<&mut TuiContext>, mut player: PlayerHandle) -> Result<Navigation> {
    let cx = cx.project();
    let mut widget = PlayerWidget {
        state: player.state().clone(),
    };
    let mut events =
        KeybindEventStream::new(cx.events, &mut widget, cx.config.keybinds.play_mpv.clone());
    loop {
        cx.term.clear()?;
        cx.term.draw_fallible(&mut events)?;
        select! {
            res = player.state_mut().changed()=> {
                if res.is_err(){
                    break;
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
        let media_item = state.current.as_ref();
        let block_area = area;
        let block = Block::bordered()
            .title("Now playing")
            .padding(Padding::uniform(1));

        let area = block.inner(block_area);
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
                info!(
                    "index: {:?} {:?}",
                    media_item.season_index, media_item.episode_index
                );
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
        block.render(block_area, buf);
        Ok(())
    }
}
