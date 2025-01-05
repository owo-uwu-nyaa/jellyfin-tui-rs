use std::task::Poll;

use color_eyre::eyre::{Context, Report};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::{
    items::{ItemType, MediaItem},
    sha::Sha256,
    user_views::UserView,
    AuthStatus, JellyfinClient,
};
use list::EntryList;
use load::HomeScreenData;
use ratatui::{prelude::Backend, Terminal};
use ratatui_image::picker::Picker;
use screen::EntryScreen;
use tracing::{debug, instrument};

use crate::{entry::Entry, image::LoadImage, NextScreen, Result, TuiContext};

mod list;
pub mod load;
mod screen;

fn create_from_media_item(
    item: MediaItem,
    client: &JellyfinClient<impl AuthStatus, impl Sha256>,
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
        .map(|(image_type, tag)| LoadImage::new(client, tag.clone(), item.id.clone(), *image_type));
    Entry::new(image, title, subtitle, NextScreen::PlayItem(item))
}

fn create_from_user_view(
    item: &UserView,
    client: &JellyfinClient<impl AuthStatus, impl Sha256>,
) -> Entry {
    let title = item.name.clone();
    let image = item
        .image_tags
        .iter()
        .flat_map(|map| map.iter())
        .next()
        .map(|(image_type, tag)| LoadImage::new(client, tag.clone(), item.id.clone(), *image_type));
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
    client: &JellyfinClient<impl AuthStatus, impl Sha256>,
) -> Option<EntryList> {
    if items.is_empty() {
        None
    } else {
        EntryList::new(
            items
                .into_iter()
                .map(|item| create_from_media_item(item, client))
                .collect(),
            title.to_string(),
        )
        .into()
    }
}

fn create_home_screen(
    mut data: HomeScreenData,
    client: &JellyfinClient<impl AuthStatus, impl Sha256>,
) -> EntryScreen {
    let entries = [
        create_from_media_item_vec(data.resume, "Continue Watching", client),
        create_from_media_item_vec(data.next_up, "Next Up", client),
        EntryList::new(
            data.views
                .iter()
                .map(|item| create_from_user_view(item, client))
                .collect(),
            "Library".to_string(),
        )
        .into(),
    ]
    .into_iter()
    .chain(data.views.iter().map(|view| {
        data.latest
            .remove(view.id.as_str())
            .and_then(|items| create_from_media_item_vec(items, view.name.as_str(), client))
    }))
    .flatten()
    .collect();
    EntryScreen::new(entries, "Home".to_string())
}

#[instrument(skip_all)]
async fn render(
    term: &mut Terminal<impl Backend>,
    picker: &Picker,
    screen: &mut EntryScreen,
) -> Report {
    futures_util::future::poll_fn(|cx| {
        let mut err = Result::Ok(());
        term.draw(|frame| {
            err = screen.render(
                frame,
                frame.area(),
                picker,
                || ratatui_image::Resize::Scale(None),
                cx,
            );
        })
        .expect("error rendering term");
        match err {
            Ok(()) => Poll::Pending,
            Err(e) => Poll::Ready(e),
        }
    })
    .await
}

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: &mut TuiContext,
    data: HomeScreenData,
) -> Result<NextScreen> {
    let mut screen = create_home_screen(data, &context.jellyfin);
    loop {
        let code = tokio::select! {
            biased;
            err = render(&mut context.term, &context.image_picker , &mut screen) => {
                break Err(err)
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
