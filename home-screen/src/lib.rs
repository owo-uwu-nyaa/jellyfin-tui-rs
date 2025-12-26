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
use ratatui_fallible_widget::TermExt;
use tracing::{debug, instrument};

use keybinds::{KeybindEvent, KeybindEventStream};

pub mod load;

fn create_from_media_item_vec(
    items: Vec<MediaItem>,
    title: &str,
    context: &TuiContext,
    images_available: &ImagesAvailable,
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
                        images_available,
                        &context.image_picker,
                        &context.stats,
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
    images_available: &ImagesAvailable,
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
                        images_available,
                        &context.image_picker,
                        &context.stats,
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
    images_available: &ImagesAvailable,
) -> Result<EntryScreen> {
    let entries = [
        create_from_media_item_vec(resume, "Continue Watching", context, images_available)
            .transpose(),
        create_from_media_item_vec(next_up, "Next Up", context, images_available).transpose(),
        create_from_user_views_vec(views.clone(), "Library", context, images_available).transpose(),
    ]
    .into_iter()
    .chain(views.iter().map(|view| {
        latest.remove(view.id.as_str()).and_then(|items| {
            create_from_media_item_vec(items, view.name.as_str(), context, images_available)
                .transpose()
        })
    }))
    .flatten()
    .collect::<Result<_>>()?;
    Ok(EntryScreen::new(
        entries,
        "Home".to_string(),
        context.image_picker.clone(),
    ))
}

pub fn handle_home_screen_data(
    context: Pin<&mut TuiContext>,
    resume: Vec<MediaItem>,
    next_up: Vec<MediaItem>,
    views: Vec<UserView>,
    latest: HashMap<String, Vec<MediaItem>>,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let screen = create_home_screen(resume, next_up, views, latest, &context, &images_available)?;
    Ok(Navigation::Replace(NextScreen::HomeScreen(
        screen,
        images_available,
    )))
}

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: Pin<&mut TuiContext>,
    mut screen: EntryScreen,
    images_available: ImagesAvailable,
) -> Result<Navigation> {
    let context = context.project();
    let mut events = KeybindEventStream::new(
        context.events,
        &mut screen,
        context.config.keybinds.home_screen.clone(),
        &context.config.help_prefixes,
    );
    loop {
        context.term.draw_fallible(&mut events)?;
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
                events.get_inner().left();
                continue;
            }
            HomeScreenCommand::Right => {
                events.get_inner().right();
            }
            HomeScreenCommand::Up => {
                events.get_inner().up();
            }
            HomeScreenCommand::Down => {
                events.get_inner().down();
            }
            HomeScreenCommand::Open => {
                if let Some(entry) = events.get_inner().get() {
                    let next = entry.open();
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenEpisode => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.episode()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenSeason => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.season()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::OpenSeries => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.series()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::Play => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.play()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::PlayOpen => {
                if let Some(entry) = events.get_inner().get() {
                    let next = entry.play_open();
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next,
                    });
                }
            }
            HomeScreenCommand::RefreshItem => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(id) = entry.item_id()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadHomeScreen,
                        next: NextScreen::RefreshItem(id.to_string()),
                    });
                }
            }
        }
    }
}
