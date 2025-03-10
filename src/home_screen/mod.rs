use color_eyre::eyre::Context;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use list::EntryList;
use load::HomeScreenData;
use screen::EntryScreen;
use tracing::{debug, instrument};

use crate::{
    entry::Entry,
    image::ImagesAvailable,
    state::{Navigation, NextScreen},
    Result, TuiContext,
};

mod list;
pub mod load;
mod screen;

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
                    }))) => code,
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => break Err(e).context("getting key events from terminal"),
                    None => break Ok(Navigation::Exit)
                }
            }
        };
        debug!("received code {code:?}");
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                break Ok(Navigation::PopContext);
            }
            KeyCode::Char('r') => {
                break Ok(Navigation::Replace(NextScreen::LoadHomeScreen));
            }
            KeyCode::Left => {
                screen.left();
            }
            KeyCode::Right => {
                screen.right();
            }
            KeyCode::Up => {
                screen.up();
            }
            KeyCode::Down => {
                screen.down();
            }
            KeyCode::Enter => {
                break Ok(Navigation::Push {
                    current: NextScreen::LoadHomeScreen,
                    next: screen.get().get_action(),
                });
            }
            _ => {}
        }
    }
}
