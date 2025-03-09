use color_eyre::eyre::Context;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::{
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use list::EntryList;
use load::HomeScreenData;
use screen::EntryScreen;
use tracing::{debug, instrument};

use crate::{
    Result, TuiContext,
    entry::Entry,
    image::{ImagesAvailable, JellyfinImageState},
    state::{Navigation, NextScreen},
};

mod list;
pub mod load;
mod screen;

fn create_from_media_item(item: MediaItem, context: &TuiContext) -> Entry {
    let (title, subtitle) = match &item.item_type {
        ItemType::Movie { container: _ } => (item.name.clone(), None),
        ItemType::Episode {
            container: _,
            season_id: _,
            season_name: _,
            series_id: _,
            series_name,
        } => (series_name.clone(), item.name.clone().into()),
        ItemType::Season {
            series_id: _,
            series_name,
        } => (series_name.clone(), item.name.clone().into()),
        ItemType::Series => (item.name.clone(), None),
    };
    let image = item
        .image_tags
        .iter()
        .flat_map(|map| map.iter())
        .next()
        .map(|(image_type, tag)| {
            JellyfinImageState::new(
                &context.jellyfin,
                context.cache.clone(),
                tag.clone(),
                item.id.clone(),
                *image_type,
                context.image_cache.clone(),
            )
        });
    Entry::new(image, title, subtitle, NextScreen::LoadPlayItem(item))
}

fn create_from_user_view(item: &UserView, context: &TuiContext) -> Entry {
    let title = item.name.clone();
    let image = item
        .image_tags
        .iter()
        .flat_map(|map| map.iter())
        .next()
        .map(|(image_type, tag)| {
            JellyfinImageState::new(
                &context.jellyfin,
                context.cache.clone(),
                tag.clone(),
                item.id.clone(),
                *image_type,
                context.image_cache.clone(),
            )
        });
    Entry::new(
        image,
        title,
        None,
        NextScreen::ShowUserView {
            id: item.id.clone(),
            kind: item.view_type,
        },
    )
}

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
                .map(|item| create_from_media_item(item, context))
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
                .map(|item| create_from_user_view(item, context))
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

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: &mut TuiContext,
    data: HomeScreenData,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let mut screen = create_home_screen(data, context);
    loop {
        context
            .term
            .draw(|frame| {
                screen.render_screen(
                    frame.area(),
                    frame.buffer_mut(),
                    &images_available,
                    &context.image_picker,
                );
            })
            .context("rendering home screen")?;
        let code = tokio::select! {
            _ = images_available.wait_available() => {continue;
            }
            term = context.events.next() => {
                match term {
                    Some(Ok(Event::Key(KeyEvent {
                        code,
                        modifiers:_,
                        kind: KeyEventKind::Press,
                        state:_,
                    }))) => Some(code),
                    Some(Ok(_)) => None,
                    Some(Err(e)) => break Err(e).context("getting key events from terminal"),
                    None => break Ok(Navigation::PopContext)
                }
            }
        };
        debug!("received code {code:?}");
        match code {
            Some(KeyCode::Char('q') | KeyCode::Esc) => {
                break Ok(Navigation::PopContext);
            }
            Some(KeyCode::Char('r')) => {
                break Ok(Navigation::Replace(NextScreen::LoadHomeScreen));
            }
            Some(KeyCode::Left) => {
                screen.left();
            }
            Some(KeyCode::Right) => {
                screen.right();
            }
            Some(KeyCode::Up) => {
                screen.up();
            }
            Some(KeyCode::Down) => {
                screen.down();
            }
            Some(KeyCode::Enter) => {
                break Ok(Navigation::Push {
                    current: NextScreen::LoadHomeScreen,
                    next: screen.get().get_action(),
                });
            }
            _ => {}
        }
    }
}
