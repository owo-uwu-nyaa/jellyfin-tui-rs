use color_eyre::eyre::{Context, Result};
use entries::{entry::Entry, grid::EntryGrid, image::available::ImagesAvailable};
use fetch::fetch_screen;
use futures_util::StreamExt;
use jellyfin::{
    Auth, JellyfinClient, JellyfinVec,
    items::{GetItemsQuery, MediaItem},
    user_views::UserView,
};
use jellyfin_tui_core::{
    context::TuiContext,
    entries::EntryExt,
    keybinds::UserViewCommand,
    state::{Navigation, NextScreen, ToNavigation},
};
use ratatui_fallible_widget::TermExt;
use std::pin::Pin;
use tracing::debug;

use keybinds::{KeybindEvent, KeybindEventStream};

async fn fetch_user_view_items(
    jellyfin: &JellyfinClient<Auth>,
    view: &UserView,
) -> Result<Vec<MediaItem>> {
    let user_id = jellyfin.get_auth().user.id.as_str();
    let items = JellyfinVec::collect(async |start| {
        jellyfin
            .get_items(&GetItemsQuery {
                user_id: user_id.into(),
                start_index: start.into(),
                limit: 100.into(),
                recursive: None,
                parent_id: view.id.as_str().into(),
                exclude_item_types: None,
                include_item_types: None,
                enable_images: true.into(),
                enable_image_types: "Thumb, Backdrop, Primary".into(),
                image_type_limit: 1.into(),
                enable_user_data: true.into(),
                fields: None,
                sort_by: "DateLastContentAdded".into(),
                sort_order: "Descending".into(),
            })
            .await
            .context("requesting items")?
            .deserialize()
            .await
            .context("deserializing items")
    })
    .await?;
    Ok(items)
}

pub async fn fetch_user_view(cx: Pin<&mut TuiContext>, view: UserView) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        &format!("Loading user view {}", view.name),
        async move {
            Ok(fetch_user_view_items(jellyfin, &view)
                .await
                .map(move |items| Navigation::Replace(NextScreen::UserView { view, items }))
                .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
        &cx.config.help_prefixes,
    )
    .await
}

pub async fn display_user_view(
    cx: Pin<&mut TuiContext>,
    view: UserView,
    items: Vec<MediaItem>,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let mut grid = EntryGrid::new(
        items
            .into_iter()
            .map(|item| {
                Entry::from_media_item(
                    item,
                    &cx.jellyfin,
                    &cx.cache,
                    &cx.image_cache,
                    &images_available,
                    &cx.image_picker,
                    &cx.stats,
                )
            })
            .collect::<Result<Vec<_>>>()?,
        view.name.clone(),
        cx.image_picker.clone(),
    );
    let cx = cx.project();
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut grid,
        cx.config.keybinds.user_view.clone(),
        &cx.config.help_prefixes,
    );
    loop {
        cx.term.draw_fallible(&mut events)?;
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
        debug!("received command {cmd:?}");
        match cmd {
            UserViewCommand::Quit => {
                break Ok(Navigation::PopContext);
            }
            UserViewCommand::Reload => {
                break Ok(Navigation::Replace(NextScreen::LoadUserView(view)));
            }
            UserViewCommand::Prev => {
                events.get_inner().left();
            }
            UserViewCommand::Next => {
                events.get_inner().right();
            }
            UserViewCommand::Up => {
                events.get_inner().up();
            }
            UserViewCommand::Down => {
                events.get_inner().down();
            }
            UserViewCommand::RefreshItem => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(id) = entry.item_id()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next: NextScreen::RefreshItem(id.to_string()),
                    });
                }
            }
            UserViewCommand::Play => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.play()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next,
                    });
                }
            }
            UserViewCommand::Open => {
                if let Some(entry) = events.get_inner().get() {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next: entry.open(),
                    });
                }
            }
            UserViewCommand::OpenEpisode => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.episode()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next,
                    });
                }
            }
            UserViewCommand::OpenSeason => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.season()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next,
                    });
                }
            }
            UserViewCommand::OpenSeries => {
                if let Some(entry) = events.get_inner().get()
                    && let Some(next) = entry.series()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next,
                    });
                }
            }
        }
    }
}
