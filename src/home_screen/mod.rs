use color_eyre::eyre::Context;
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use list::EntryList;
use load::HomeScreenData;
use screen::EntryScreen;
use tracing::{debug, instrument};

use crate::{
    entry::Entry,
    image::ImagesAvailable,
    keybinds::{Command, KeybindEvent, KeybindEventStream},
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

#[derive(Debug, Clone, Copy)]
pub enum HomeScreenCommand {
    Quit,
    Reload,
    Left,
    Right,
    Up,
    Down,
    Open,
}

impl Command for HomeScreenCommand {
    fn name(self) -> &'static str {
        match self {
            HomeScreenCommand::Quit => "quit",
            HomeScreenCommand::Reload => "reload",
            HomeScreenCommand::Left => "left",
            HomeScreenCommand::Right => "right",
            HomeScreenCommand::Up => "up",
            HomeScreenCommand::Down => "down",
            HomeScreenCommand::Open => "open",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "quit" => HomeScreenCommand::Quit.into(),
            "reload" => HomeScreenCommand::Reload.into(),
            "left" => HomeScreenCommand::Left.into(),
            "right" => HomeScreenCommand::Right.into(),
            "up" => HomeScreenCommand::Up.into(),
            "down" => HomeScreenCommand::Down.into(),
            "open" => HomeScreenCommand::Open.into(),
            _ => None,
        }
    }
}

#[instrument(skip_all)]
pub async fn display_home_screen(
    context: &mut TuiContext,
    data: HomeScreenData,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let mut screen = create_home_screen(data, context);
    let mut events = KeybindEventStream::new(
        &mut context.events,
        context.config.keybinds.home_screen.clone(),
    );
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
        let cmd = tokio::select! {
            _ = images_available.wait_available() => {continue;
            }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(cmd))) => cmd,
                    Some(Ok(KeybindEvent::Text(_))) => unimplemented!(),
                    Some(Ok(KeybindEvent::Render)) => continue,
                    Some(Err(e)) => break Err(e).context("getting key events from terminal"),
                    None => break Ok(Navigation::Exit)
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
                break Ok(Navigation::Push {
                    current: NextScreen::LoadHomeScreen,
                    next: screen.get().get_action(),
                });
            }
        }
    }
}
