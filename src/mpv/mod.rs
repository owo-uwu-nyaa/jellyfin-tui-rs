pub mod fetch_items;
mod log;
mod mpv_stream;
mod player;

use std::{borrow::Cow, io::Stdout};

use color_eyre::eyre::{Context, Result};
use futures_util::{StreamExt, TryStreamExt};
use jellyfin::items::MediaItem;
use player::{Player, PlayerState};
use ratatui::{
    Terminal,
    layout::{Constraint, Layout},
    prelude::CrosstermBackend,
    widgets::{Block, Padding, Paragraph},
};
use tokio::select;
use tracing::{info, instrument};

use crate::{
    TuiContext,
    keybinds::{Command, KeybindEvent, KeybindEventStream},
    state::{Navigation, NextScreen},
};

#[derive(Debug, Clone, Copy)]
pub enum MpvCommand {
    Quit,
}

impl Command for MpvCommand {
    fn name(self) -> &'static str {
        match self {
            MpvCommand::Quit => "quit",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "quit" => Some(MpvCommand::Quit),
            _ => None,
        }
    }
}

#[instrument(skip_all)]
pub async fn play(cx: &mut TuiContext, items: Vec<MediaItem>, index: usize) -> Result<Navigation> {
    if items.is_empty() {
        return Ok(Navigation::Replace(NextScreen::Error(
            "Unable to play, item is empty".into(),
        )));
    }
    let mut player = Player::new(cx, &cx.jellyfin, items, index).await?;
    let mut events = KeybindEventStream::new(&mut cx.events, cx.config.keybinds.play_mpv.clone());
    loop {
        cx.term.clear()?;
        render(&mut cx.term, player.state()?, &mut events)?;
        select! {
            res = player.try_next() => {
                if res?.is_none(){
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
    state: PlayerState<'_>,
    events: &mut KeybindEventStream<'_, MpvCommand>,
) -> Result<()> {
    match state {
        PlayerState::Initializing => {
            term.draw(|frame| {
                frame.render_widget(
                    Paragraph::new("loading").centered(),
                    events.inner(frame.area()),
                );
                frame.render_widget(events, frame.area());
            })
            .context("rendering loading screen")?;
        }
        PlayerState::Playing(media_item) => {
            term.draw(|frame| {
                let block_area = events.inner(frame.area());
                let block = Block::bordered()
                    .title("Now playing")
                    .padding(Padding::uniform(1));

                let area = block.inner(block_area);
                match &media_item.item_type {
                    jellyfin::items::ItemType::Movie { container: _ } => {
                        frame.render_widget(
                            Paragraph::new(media_item.name.clone()).centered(),
                            area,
                        );
                    }
                    jellyfin::items::ItemType::Episode {
                        container: _,
                        season_id: _,
                        season_name: None,
                        series_id: _,
                        series_name,
                        episode_index,
                        seasion_index,
                    } => {
                        let [series, episode] =
                            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                                .vertical_margin(3)
                                .areas(area);
                        let mut series_str = Cow::from(series_name.as_str());
                        if episode_index.is_some() || seasion_index.is_some() {
                            series_str.to_mut().push(' ');
                            if let Some(season) = seasion_index {
                                series_str.to_mut().push('S');
                                series_str.to_mut().push_str(&season.to_string());
                            }
                            if let Some(episode) = episode_index {
                                series_str.to_mut().push('E');
                                series_str.to_mut().push_str(&episode.to_string());
                            }
                        }

                        frame.render_widget(Paragraph::new(series_str).centered(), series);
                        frame.render_widget(
                            Paragraph::new(media_item.name.clone()).centered(),
                            episode,
                        );
                    }
                    jellyfin::items::ItemType::Episode {
                        container: _,
                        season_id: _,
                        season_name: Some(season_name),
                        series_id: _,
                        series_name,
                        episode_index,
                        seasion_index,
                    } => {
                        let [series, season, episode] = Layout::vertical([
                            Constraint::Fill(1),
                            Constraint::Fill(1),
                            Constraint::Fill(1),
                        ])
                        .vertical_margin(3)
                        .areas(area);
                        let mut series_str = Cow::from(series_name.as_str());
                        if episode_index.is_some() || seasion_index.is_some() {
                            series_str.to_mut().push(' ');
                            if let Some(season) = seasion_index {
                                series_str.to_mut().push('S');
                                series_str.to_mut().push_str(&season.to_string());
                            }
                            if let Some(episode) = episode_index {
                                series_str.to_mut().push('E');
                                series_str.to_mut().push_str(&episode.to_string());
                            }
                        }
                        frame.render_widget(Paragraph::new(series_str).centered(), series);
                        frame.render_widget(Paragraph::new(season_name.clone()).centered(), season);
                        frame.render_widget(
                            Paragraph::new(media_item.name.clone()).centered(),
                            episode,
                        );
                    }
                    _ => {
                        panic!("unexpected media item type: {media_item:#?}");
                    }
                }
                frame.render_widget(block, block_area);
                frame.render_widget(events, frame.area());
            })
            .context("rendering playing item")?;
        }
        PlayerState::Exiting => {
            term.draw(|frame| {
                frame.render_widget(
                    Paragraph::new("quitting").centered(),
                    events.inner(frame.area()),
                );
                frame.render_widget(events, frame.area());
            })
            .context("rendering exit screen")?;
        }
    }
    Ok(())
}
