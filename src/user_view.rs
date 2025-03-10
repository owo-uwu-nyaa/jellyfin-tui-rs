use color_eyre::eyre::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::{
    items::{GetItemsQuery, MediaItem},
    sha::Sha256,
    user_views::UserView,
    Auth, JellyfinClient, JellyfinVec,
};
use ratatui::widgets::{Block, Paragraph};
use std::pin::pin;
use tracing::debug;

use crate::{
    entry::Entry,
    grid::EntryGrid,
    image::ImagesAvailable,
    state::{Navigation, NextScreen},
    TuiContext,
};

async fn fetch_user_view_items(
    jellyfin: &JellyfinClient<Auth, impl Sha256>,
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
                enable_image_types: "Primary, Backdrop, Thumb".into(),
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
    loop {
        cx.term
            .draw(|frame| frame.render_widget(&msg, frame.area()))
            .context("rendering ui")?;
        tokio::select! {
            data = &mut fetch => {
                let items = data.with_context(||format!("loading user view {}", view.name))?;
                break Ok(Navigation::Replace(NextScreen::UserView { view:view.clone() , items  }))
            }
            term = cx.events.next() => {
                match term {
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('q')| KeyCode::Esc,
                        modifiers: _,
                        kind: KeyEventKind::Press,
                        state: _,
                    })))
                        | None => break Ok(Navigation::PopContext),
                    Some(Ok(_)) => {
                        cx.term
                          .draw(|frame| frame.render_widget(&msg, frame.area()))
                          .context("rendering ui")?;
                    }
                    Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
                }
            }
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
    loop {
        cx.term
            .draw(|frame| {
                grid.render(
                    frame.area(),
                    frame.buffer_mut(),
                    &images_available,
                    &cx.image_picker,
                );
            })
            .context("drawing user view")?;
        let code = tokio::select! {
            _ = images_available.wait_available() => {continue;
            }
            term = cx.events.next() => {
                match term {
                    Some(Ok(Event::Key(KeyEvent {
                        code,
                        modifiers:_,
                        kind: KeyEventKind::Press,
                        state:_,
                    }))) => code,
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => break Err(e).context("getting key events from terminal"),
                    None => break Ok(Navigation::PopContext)
                }
            }
        };
        debug!("received code {code:?}");
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                break Ok(Navigation::PopContext);
            }
            KeyCode::Char('r') => {
                break Ok(Navigation::Replace(NextScreen::LoadUserView(view)));
            }
            KeyCode::Left => {
                grid.left();
            }
            KeyCode::Right => {
                grid.right();
            }
            KeyCode::Up => {
                grid.up();
            }
            KeyCode::Down => {
                grid.down();
            }
            KeyCode::Enter => {
                break Ok(Navigation::Push {
                    current: NextScreen::LoadUserView(view),
                    next: grid.get().get_action(),
                });
            }
            _ => {}
        }
    }
}
