pub mod fetch_items;
mod log;
mod mpv_stream;
mod player;

use std::io::Stdout;

use color_eyre::eyre::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
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
use tracing::instrument;

use crate::{TuiContext, state::Navigation};

#[instrument(skip_all)]
pub async fn play(cx: &mut TuiContext, items: Vec<MediaItem>, index: usize) -> Result<Navigation> {
    let mut player = Player::new(cx, &cx.jellyfin, items, index)?;
    loop {
        cx.term.clear()?;
        render(&mut cx.term, player.state()?)?;
        select! {
            res = player.try_next() => {
                if res?.is_none(){
                    break;
                }
            }
            event = cx.events.next() => {
                match event {
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('q') | KeyCode::Esc,
                        modifiers:_,
                        kind: KeyEventKind::Press,
                        state:_,
                    }))) => {break;}
                    Some(Ok(_)) => {},
                    Some(Err(e)) => return Err(e).context("getting key events from terminal"),
                    None => {break;}
                }
            }
        }
    }
    while let Some(()) = player.try_next().await? {}
    //some ffmpeg stuff still writes to stdout
    cx.term.clear()?;
    Ok(Navigation::PopContext)
}

fn render(term: &mut Terminal<CrosstermBackend<Stdout>>, state: PlayerState<'_>) -> Result<()> {
    match state {
        PlayerState::Initializing => {
            term.draw(|frame| {
                frame.render_widget(Paragraph::new("loading").centered(), frame.area());
            })
            .context("rendering loading screen")?;
        }
        PlayerState::Playing(media_item) => {
            term.draw(|frame| {
                let block = Block::bordered()
                    .title("Now playing")
                    .padding(Padding::uniform(1));
                let area = block.inner(frame.area());
                frame.render_widget(block, frame.area());
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
                    } => {
                        let [series, episode] =
                            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                                .vertical_margin(3)
                                .areas(area);
                        frame.render_widget(Paragraph::new(series_name.clone()).centered(), series);
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
                    } => {
                        let [series, season, episode] = Layout::vertical([
                            Constraint::Fill(1),
                            Constraint::Fill(1),
                            Constraint::Fill(1),
                        ])
                        .vertical_margin(3)
                        .areas(area);
                        frame.render_widget(Paragraph::new(series_name.clone()).centered(), series);
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
            })
            .context("rendering playing item")?;
        }
        PlayerState::Exiting => {
            term.draw(|frame| {
                frame.render_widget(Paragraph::new("quitting").centered(), frame.area());
            })
            .context("rendering exit screen")?;
        }
    }
    Ok(())
}
