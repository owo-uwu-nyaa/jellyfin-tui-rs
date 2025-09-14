use std::{collections::HashMap, pin::Pin};

use color_eyre::eyre::{Context, Result};
use entries::{
    entry::Entry, image::available::ImagesAvailable, list::EntryList, screen::EntryScreen,
};
use futures_util::StreamExt;
use jellyfin::{items::MediaItem, user_views::UserView};
use jellyfin_tui_core::{
    context::TuiContext,
    entries::EntryExt,
    keybinds::HomeScreenCommand,
    state::{Navigation, NextScreen},
};
use ratatui::widgets::Widget;
use tracing::{debug, instrument};

use keybinds::{KeybindEvent, KeybindEventStream};

pub mod load;

fn create_from_media_item_vec(
    items: Vec<MediaItem>,
    title: &str,
    context: &TuiContext,
) -> Result<Option<EntryList>> {
    Ok(if items.is_empty() {
        None
    } else {
        EntryList::new(
            items
                .into_iter()
                .map(|item| {
                    Entry::from_media_item(
                        item,
                        &context.jellyfin,
                        &context.cache,
                        &context.image_cache,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
            title.to_string(),
        )
        .into()
    })
}

fn create_from_user_views_vec(
    items: Vec<UserView>,
    title: &str,
    context: &TuiContext,
) -> Result<Option<EntryList>> {
    Ok(if items.is_empty() {
        None
    } else {
        EntryList::new(
            items
                .into_iter()
                .map(|item| {
                    Entry::from_user_view(
                        item,
                        &context.jellyfin,
                        &context.cache,
                        &context.image_cache,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
            title.to_string(),
        )
        .into()
    })
}

fn create_home_screen(
    resume: Vec<MediaItem>,
    next_up: Vec<MediaItem>,
    views: Vec<UserView>,
    mut latest: HashMap<String, Vec<MediaItem>>,
    context: &TuiContext,
) -> Result<EntryScreen> {
    let entries = [
        create_from_media_item_vec(resume, "Continue Watching", context).transpose(),
        create_from_media_item_vec(next_up, "Next Up", context).transpose(),
        create_from_user_views_vec(views.clone(), "Library", context).transpose(),
    ]
    .into_iter()
    .chain(views.iter().map(|view| {
        latest.remove(view.id.as_str()).and_then(|items| {
            create_from_media_item_vec(items, view.name.as_str(), context).transpose()
        })
    }))
    .flatten()
    .collect::<Result<_>>()?;
    Ok(EntryScreen::new(entries, "Home".to_string()))
}

pub fn handle_home_screen_data(
    context: Pin<&mut TuiContext>,
    resume: Vec<MediaItem>,
    next_up: Vec<MediaItem>,
    views: Vec<UserView>,
    latest: HashMap<String, Vec<MediaItem>>,
) -> Result<Navigation> {
    Ok(Navigation::Replace(NextScreen::HomeScreen(
        create_home_screen(resume, next_up, views, latest, &context)?,
    )))
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
                break Ok(Navigation::Replace(NextScreen::LoadHomeScreen));
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
                if let Some(entry) = screen.get()
                    && let Some(next) = entry.episode()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenSeason => {
                if let Some(entry) = screen.get()
                    && let Some(next) = entry.season()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenSeries => {
                if let Some(entry) = screen.get()
                    && let Some(next) = entry.series()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
                }
            }
            HomeScreenCommand::Play => {
                if let Some(entry) = screen.get()
                    && let Some(next) = entry.play()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::HomeScreen(screen),
                        next,
                    });
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
