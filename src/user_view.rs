use color_eyre::eyre::{Context, Result};
use futures_util::StreamExt;
use jellyfin::{
    items::{GetItemsQuery, MediaItem},
    sha::ShaImpl,
    user_views::UserView,
    Auth, JellyfinClient, JellyfinVec,
};
use std::pin::Pin;
use tracing::debug;

use crate::{
    entry::Entry,
    fetch::fetch_screen,
    grid::EntryGrid,
    image::ImagesAvailable,
    state::{Navigation, NextScreen, ToNavigation},
    TuiContext,
};
use keybinds::{Command, KeybindEvent, KeybindEventStream};

async fn fetch_user_view_items(
    jellyfin: &JellyfinClient<Auth, impl ShaImpl>,
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
    )
    .await
}

#[derive(Debug, Clone, Copy, Command)]
pub enum UserViewCommand {
    Quit,
    Reload,
    Prev,
    Next,
    Up,
    Down,
    Open,
    Play,
    OpenEpisode,
    OpenSeason,
    OpenSeries,
}

pub async fn display_user_view(
    cx: Pin<&mut TuiContext>,
    view: UserView,
    items: Vec<MediaItem>,
) -> Result<Navigation> {
    let mut grid = EntryGrid::new(
        items
            .into_iter()
            .map(|item| Entry::from_media_item(item, &cx))
            .collect(),
        view.name.clone(),
    );
    let images_available = ImagesAvailable::new();
    let cx = cx.project();
    let mut events = KeybindEventStream::new(cx.events, cx.config.keybinds.user_view.clone());
    loop {
        cx.term
            .draw(|frame| {
                grid.render(
                    events.inner(frame.area()),
                    frame.buffer_mut(),
                    &images_available,
                    cx.image_picker,
                );
                frame.render_widget(&mut events, frame.area());
            })
            .context("drawing user view")?;
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
                grid.left();
            }
            UserViewCommand::Next => {
                grid.right();
            }
            UserViewCommand::Up => {
                grid.up();
            }
            UserViewCommand::Down => {
                grid.down();
            }
            UserViewCommand::Play => {
                if let Some(entry) = grid.get() {
                    if let Some(next) = entry.play() {
                        break Ok(Navigation::Push {
                            current: NextScreen::LoadUserView(view),
                            next,
                        });
                    }
                }
            }
            UserViewCommand::Open => {
                if let Some(entry) = grid.get() {
                    break Ok(Navigation::Push {
                        current: NextScreen::LoadUserView(view),
                        next: entry.open(),
                    });
                }
            }
            UserViewCommand::OpenEpisode => {
                if let Some(entry) = grid.get() {
                    if let Some(next) = entry.episode() {
                        break Ok(Navigation::Push {
                            current: NextScreen::LoadHomeScreen,
                            next,
                        });
                    }
                }
            }
            UserViewCommand::OpenSeason => {
                if let Some(entry) = grid.get() {
                    if let Some(next) = entry.season() {
                        break Ok(Navigation::Push {
                            current: NextScreen::LoadHomeScreen,
                            next,
                        });
                    }
                }
            }
            UserViewCommand::OpenSeries => {
                if let Some(entry) = grid.get() {
                    if let Some(next) = entry.series() {
                        break Ok(Navigation::Push {
                            current: NextScreen::LoadHomeScreen,
                            next,
                        });
                    }
                }
            }
        }
    }
}
