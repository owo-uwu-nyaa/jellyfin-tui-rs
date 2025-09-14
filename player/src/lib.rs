pub mod fetch_items;

use std::{borrow::Cow, io::Stdout, pin::Pin};

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
    Terminal,
    layout::{Constraint, Layout},
    prelude::CrosstermBackend,
    widgets::{Block, Padding, Paragraph},
};
use spawn::spawn;
use tokio::select;
use tracing::{error_span, info, instrument};

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
    let mut events = KeybindEventStream::new(cx.events, cx.config.keybinds.play_mpv.clone());
    loop {
        cx.term.clear()?;
        render(cx.term, &player.state().borrow(), &mut events)?;

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

fn render(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &PlayerState,
    events: &mut KeybindEventStream<'_, MpvCommand>,
) -> Result<()> {
    term.draw(|frame| {
        let media_item = state.current.as_ref();
        let block_area = events.inner(frame.area());
        let block = Block::bordered()
            .title("Now playing")
            .padding(Padding::uniform(1));

        let area = block.inner(block_area);
        match &media_item.item_type {
            jellyfin::items::ItemType::Movie => {
                frame.render_widget(Paragraph::new(media_item.name.clone()).centered(), area);
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

                frame.render_widget(Paragraph::new(series_str).centered(), series);
                frame.render_widget(Paragraph::new(media_item.name.clone()).centered(), episode);
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
                frame.render_widget(Paragraph::new(series_str).centered(), series);
                frame.render_widget(Paragraph::new(season_name.clone()).centered(), season);
                frame.render_widget(Paragraph::new(media_item.name.clone()).centered(), episode);
            }
            _ => {
                panic!("unexpected media item type: {media_item:#?}");
            }
        }
        frame.render_widget(block, block_area);
        frame.render_widget(events, frame.area());
    })
    .context("rendering playing item")?;
    Ok(())
}
