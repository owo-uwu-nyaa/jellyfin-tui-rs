
use color_eyre::eyre::Context;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::{
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use list::EntryList;
use load::HomeScreenData;
use ratatui::{prelude::Backend, Terminal};
use ratatui_image::picker::Picker;
use screen::EntryScreen;
use tracing::{debug, instrument};

use crate::{
    entry::Entry,
    image::{ImagesAvailable, JellyfinImageState},
    NextScreen, Result, TuiContext,
};

mod list;
pub mod load;
mod screen;

fn create_from_media_item(
    item: MediaItem,
    context: &TuiContext,
    images_availabe: &ImagesAvailable,
) -> Entry {
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
                images_availabe,
                tag.clone(),
                item.id.clone(),
                *image_type,
            )
        });
    Entry::new(image, title, subtitle, NextScreen::PlayItem(item))
}

fn create_from_user_view(
    item: &UserView,
    context: &TuiContext,
    images_availabe: &ImagesAvailable,
) -> Entry {
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
                images_availabe,
                tag.clone(),
                item.id.clone(),
                *image_type,
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
    images_availabe: &ImagesAvailable,
) -> Option<EntryList> {
    if items.is_empty() {
        None
    } else {
        EntryList::new(
            items
                .into_iter()
                .map(|item| create_from_media_item(item, context, images_availabe))
                .collect(),
            title.to_string(),
        )
        .into()
    }
}

fn create_home_screen(mut data: HomeScreenData, context: &TuiContext,
    images_availabe: &ImagesAvailable,
) -> EntryScreen {
    let entries = [
        create_from_media_item_vec(data.resume, "Continue Watching", context, images_availabe),
        create_from_media_item_vec(data.next_up, "Next Up", context,images_availabe),
        EntryList::new(
            data.views
                .iter()
                .map(|item| create_from_user_view(item, context,images_availabe))
                .collect(),
            "Library".to_string(),
        )
        .into(),
    ]
    .into_iter()
    .chain(data.views.iter().map(|view| {
        data.latest
            .remove(view.id.as_str())
            .and_then(|items| create_from_media_item_vec(items, view.name.as_str(), context, images_availabe))
    }))
    .flatten()
    .collect();
    EntryScreen::new(entries, "Home".to_string())
}

#[instrument(skip_all)]
fn render(
    term: &mut Terminal<impl Backend>,
    screen: &mut EntryScreen,
    availabe: &ImagesAvailable,
    picker: &Picker,
) -> Result<()> {
    let mut res = Result::Ok(());
    term.draw(|frame| {
        res = screen.render_screen(frame.area(), frame.buffer_mut(), availabe, picker);
    }).context("rendering home screen")?;
    res
}

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: &mut TuiContext,
    data: HomeScreenData,
) -> Result<NextScreen> {
    let images_available = ImagesAvailable::new();
    let mut screen = create_home_screen(data, context, &images_available);
    loop {
        render(
            &mut context.term,
            &mut screen,
            &images_available,
            &context.image_picker,
        )?;
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
                    None => break Ok(NextScreen::Quit)
                }
            }
        };
        debug!("received code {code:?}");
        match code {
            Some(KeyCode::Char('q') | KeyCode::Esc) => {
                break Ok(NextScreen::Quit);
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
                break Ok(screen.get());
            }
            _ => {}
        }
    }
}
