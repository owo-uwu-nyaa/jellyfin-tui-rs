use std::pin::Pin;

use crate::list::EntryList;
use crate::screen::EntryScreen;
use color_eyre::eyre::Context;
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use load::HomeScreenData;
use ratatui::widgets::Widget;
use tracing::{debug, instrument};

use crate::{
    entry::Entry,
    image::ImagesAvailable,
    state::{Navigation, NextScreen},
    Result, TuiContext,
};
use keybinds::{Command, KeybindEvent, KeybindEventStream};

pub mod load;

fn create_from_media_item_vec(
    items: Vec<MediaItem>,
    title: &str,
    context: &TuiContext,
) -> Option<EntryList> {
    if items.is_empty() {
        None
    } else {
        EntryList::new(
            items
                .into_iter()
                .map(|item| Entry::from_media_item(item, context))
                .collect(),
            title.to_string(),
        )
        .into()
    }
}

fn create_home_screen(mut data: HomeScreenData, context: &TuiContext) -> EntryScreen {
    let entries = [
        create_from_media_item_vec(data.resume, "Continue Watching", context),
        create_from_media_item_vec(data.next_up, "Next Up", context),
        EntryList::new(
            data.views
                .iter()
                .cloned()
                .map(|item| Entry::from_user_view(item, context))
                .collect(),
            "Library".to_string(),
        )
        .into(),
    ]
    .into_iter()
    .chain(data.views.iter().map(|view| {
        data.latest
            .remove(view.id.as_str())
            .and_then(|items| create_from_media_item_vec(items, view.name.as_str(), context))
    }))
    .flatten()
    .collect();
    EntryScreen::new(entries, "Home".to_string())
}

#[derive(Debug, Clone, Copy, Command)]
pub enum HomeScreenCommand {
    Quit,
    Reload,
    Left,
    Right,
    Up,
    Down,
    Open,
    Play,
    PlayOpen,
    OpenEpisode,
    OpenSeason,
    OpenSeries,
}

pub async fn handle_home_screen_data(
    context: Pin<&mut TuiContext>,
    data: HomeScreenData,
) -> Result<Navigation> {
    Ok(Navigation::Replace(NextScreen::HomeScreen(create_home_screen(data, &context))))
}

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: Pin<&mut TuiContext>,
    mut screen: EntryScreen,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let context = context.project();
    let mut events =
        KeybindEventStream::new(context.events, context.config.keybinds.home_screen.clone());
    loop {
        context
            .term
            .draw(|frame| {
                let area = events.inner(frame.area());
                screen.render(
                    area,
                    frame.buffer_mut(),
                    &images_available,
                    context.image_picker,
                );
                events.render(frame.area(), frame.buffer_mut());
            })
            .context("rendering home screen")?;
        let cmd = tokio::select! {
            _ = images_available.wait_available() => {continue ;
            }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(cmd))) => cmd,
                    Some(Ok(KeybindEvent::Text(_))) => unimplemented!(),
                    Some(Ok(KeybindEvent::Render)) => continue ,
                    Some(Err(e)) => break  Err(e).context("getting key events from terminal"),
                    None => break  Ok(Navigation::Exit)
                }
            }
        };
        debug!("received command {cmd:?}");
        match cmd {
            HomeScreenCommand::Quit => {
                break Ok(Navigation::PopContext);
            }
            HomeScreenCommand::Reload => {
                break Ok(Navigation::Replace(NextScreen::HomeScreen(screen)));
            }
            HomeScreenCommand::Left => {
                screen.left();
                continue;
            }
            HomeScreenCommand::Right => {
                screen.right();
            }
            HomeScreenCommand::Up => {
                screen.up();
            }
            HomeScreenCommand::Down => {
                screen.down();
            }
            HomeScreenCommand::Open => {
                if let Some(entry) = screen.get() {
                    let next = entry.open();
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenEpisode => {
                if let Some(entry) = screen.get() {
                    if let Some(next) = entry.episode() {
                        break Ok(Navigation::Push {
                            current: NextScreen::HomeScreen(screen),
                            next,
                        });
                    }
                }
            }
            HomeScreenCommand::OpenSeason => {
                if let Some(entry) = screen.get() {
                    if let Some(next) = entry.season() {
                        break Ok(Navigation::Push {
                            current: NextScreen::HomeScreen(screen),
                            next,
                        });
                    }
                }
            }
            HomeScreenCommand::OpenSeries => {
                if let Some(entry) = screen.get() {
                    if let Some(next) = entry.series() {
                        break Ok(Navigation::Push {
                            current: NextScreen::HomeScreen(screen),
                            next,
                        });
                    }
                }
            }
            HomeScreenCommand::Play => {
                if let Some(entry) = screen.get() {
                    if let Some(next) = entry.play() {
                        break Ok(Navigation::Push {
                            current: NextScreen::HomeScreen(screen),
                            next,
                        });
                    }
                }
            }
            HomeScreenCommand::PlayOpen => {
                if let Some(entry) = screen.get() {
                    let next = entry.play_open();
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
                }
            }
        }
    }
}
