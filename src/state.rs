use std::borrow::Cow;

use futures_util::StreamExt;
use jellyfin::{items::MediaItem, user_views::UserView};
use ratatui::widgets::{Block, Paragraph, Wrap};
use tracing::debug;

use crate::{
    home_screen::{
        display_home_screen,
        load::{load_home_screen, HomeScreenData},
    },
    keybinds::{Command, KeybindEvent, KeybindEventStream},
    mpv,
    user_view::{display_user_view, fetch_user_view},
    TuiContext,
};
use color_eyre::{eyre::Context, Result};

#[derive(Debug)]
pub enum NextScreen {
    LoadHomeScreen,
    HomeScreen(HomeScreenData),
    LoadUserView(UserView),
    UserView {
        view: UserView,
        items: Vec<MediaItem>,
    },
    LoadPlayItem(MediaItem),
    PlayItem {
        items: Vec<MediaItem>,
        index: usize,
    },
    Error(Cow<'static, str>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Navigation {
    PopContext,
    Push {
        current: NextScreen,
        next: NextScreen,
    },
    Replace(NextScreen),
    Exit,
}

impl NextScreen {
    pub async fn show(self, cx: &mut TuiContext) -> Result<Navigation> {
        match self {
            NextScreen::LoadHomeScreen => load_home_screen(cx).await,
            NextScreen::HomeScreen(data) => display_home_screen(cx, data).await,
            NextScreen::LoadUserView(view) => fetch_user_view(cx, view).await,
            NextScreen::UserView { view, items } => display_user_view(cx, view, items).await,
            NextScreen::LoadPlayItem(media_item) => {
                mpv::fetch_items::fetch_screen(cx, media_item).await
            }
            NextScreen::PlayItem { items, index } => mpv::play(cx, items, index).await,
            NextScreen::Error(msg) => render_error(cx, msg).await,
            screen => todo!("{screen:?}"),
        }
    }
}

pub struct State {
    screen_stack: Vec<NextScreen>,
}

impl State {
    pub fn navigate(&mut self, nav: Navigation) {
        match nav {
            Navigation::PopContext => {}
            Navigation::Replace(next) => {
                self.screen_stack.push(next);
            }
            Navigation::Push { current, next } => {
                self.screen_stack.push(current);
                self.screen_stack.push(next);
            }
            Navigation::Exit => {
                debug!("full exit returned");
                self.screen_stack.clear();
            }
        }
    }
    pub fn pop(&mut self) -> Option<NextScreen> {
        self.screen_stack.pop()
    }
    pub fn new() -> Self {
        let mut stack = Vec::with_capacity(8);
        stack.push(NextScreen::LoadHomeScreen);
        Self {
            screen_stack: stack,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCommand {
    Quit,
    Kill,
}
impl Command for ErrorCommand {
    fn name(self) -> &'static str {
        match self {
            ErrorCommand::Quit => "quit",
            ErrorCommand::Kill => "kill",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "quit" => ErrorCommand::Quit.into(),
            "kill" => ErrorCommand::Kill.into(),
            _ => None,
        }
    }
}

async fn render_error(cx: &mut TuiContext, msg: Cow<'static, str>) -> Result<Navigation> {
    let msg = Paragraph::new(msg)
        .wrap(Wrap { trim: false })
        .block(Block::bordered());
    let mut events = KeybindEventStream::new(&mut cx.events, cx.config.keybinds.error.clone());
    loop {
        cx.term
            .draw(|frame| {
                frame.render_widget(&msg, events.inner(frame.area()));
                frame.render_widget(&mut events, frame.area());
            })
            .context("rendering error")?;
        match events.next().await {
            Some(Ok(KeybindEvent::Render)) => continue,
            Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
            Some(Ok(KeybindEvent::Command(ErrorCommand::Quit))) => {
                break Ok(Navigation::PopContext);
            }
            Some(Ok(KeybindEvent::Command(ErrorCommand::Kill))) | None => {
                break Ok(Navigation::Exit);
            }
            Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
        }
    }
}
