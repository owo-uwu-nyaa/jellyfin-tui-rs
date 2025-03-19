use color_eyre::eyre::{Context, Result};
use futures_util::StreamExt;
use jellyfin::{
    items::{GetItemsQuery, MediaItem},
    sha::ShaImpl,
    user_views::UserView,
    Auth, JellyfinClient, JellyfinVec,
};
use ratatui::widgets::{Block, Paragraph};
use std::pin::pin;
use tracing::{debug, error};

use crate::{
    entry::Entry,
    grid::EntryGrid,
    image::ImagesAvailable,
    keybinds::{Command, KeybindEvent, KeybindEventStream, LoadingCommand},
    state::{Navigation, NextScreen},
    TuiContext,
};

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

pub async fn fetch_user_view(cx: &mut TuiContext, view: UserView) -> Result<Navigation> {
    let msg = Paragraph::new(format!("Loading user view {}", view.name))
        .centered()
        .block(Block::bordered());
    let mut fetch = pin!(fetch_user_view_items(&cx.jellyfin, &view));
    let mut events =
        KeybindEventStream::new(&mut cx.events, cx.config.keybinds.fetch_user_view.clone());
    loop {
        cx.term
            .draw(|frame| {
                frame.render_widget(&msg, events.inner(frame.area()));
                frame.render_widget(&mut events, frame.area());
            })
            .context("rendering ui")?;
        tokio::select! {
            data = &mut fetch => {
                break match data{
                    Err(e) => {
                        error!("Error loading user view {}: {e:?}", view.name);
                        Ok(Navigation::Replace(NextScreen::Error(format!("Error loading user view {}", view.name).into())))
                    }
                    Ok(items) => Ok(Navigation::Replace(NextScreen::UserView { view:view.clone() , items  }))
                }
            }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(LoadingCommand::Quit))) => break Ok(Navigation::PopContext),
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Ok(KeybindEvent::Render)) => {
                        continue
                    }
                    Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
                    None => break Ok(Navigation::Exit)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UserViewCommand {
    Quit,
    Reload,
    Prev,
    Next,
    Up,
    Down,
    Open,
}

impl Command for UserViewCommand {
    fn name(self) -> &'static str {
        match self {
            UserViewCommand::Quit => "quit",
            UserViewCommand::Reload => "reload",
            UserViewCommand::Prev => "prev",
            UserViewCommand::Next => "next",
            UserViewCommand::Up => "up",
            UserViewCommand::Down => "down",
            UserViewCommand::Open => "open",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "quit" => UserViewCommand::Quit.into(),
            "reload" => UserViewCommand::Reload.into(),
            "prev" => UserViewCommand::Prev.into(),
            "next" => UserViewCommand::Next.into(),
            "up" => UserViewCommand::Up.into(),
            "down" => UserViewCommand::Down.into(),
            "open" => UserViewCommand::Open.into(),
            _ => None,
        }
    }
}

pub async fn display_user_view(
    cx: &mut TuiContext,
    view: UserView,
    items: Vec<MediaItem>,
) -> Result<Navigation> {
    let mut grid = EntryGrid::new(
        items
            .into_iter()
            .map(|item| Entry::from_media_item(item, cx))
            .collect(),
        view.name.clone(),
    );
    let images_available = ImagesAvailable::new();
    let mut events = KeybindEventStream::new(&mut cx.events, cx.config.keybinds.user_view.clone());
    loop {
        cx.term
            .draw(|frame| {
                grid.render(
                    events.inner(frame.area()),
                    frame.buffer_mut(),
                    &images_available,
                    &cx.image_picker,
                );
                frame.render_widget(&mut events, frame.area());
            })
            .context("drawing user view")?;
        let cmd = tokio::select! {
            _ = images_available.wait_available() => {continue;
            }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(cmd))) => cmd,
                    Some(Ok(KeybindEvent::Render)) => continue,
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Err(e)) => break Err(e).context("getting key events from terminal"),
                    None => break Ok(Navigation::PopContext)
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
            UserViewCommand::Open => {
                break Ok(Navigation::Push {
                    current: NextScreen::LoadUserView(view),
                    next: grid.get().get_action(),
                });
            }
        }
    }
}
