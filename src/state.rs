use jellyfin::{items::MediaItem, user_views::UserViewType};

use crate::{
    TuiContext,
    home_screen::{
        display_home_screen,
        load::{HomeScreenData, load_home_screen},
    },
};
use color_eyre::Result;

#[derive(Debug)]
pub enum NextScreen {
    LoadHomeScreen,
    HomeScreen(HomeScreenData),
    ShowUserView { id: String, kind: UserViewType },
    PlayItem(MediaItem),
}

#[derive(Debug)]
pub enum Navigation {
    PopContext,
    Push {
        current: NextScreen,
        next: NextScreen,
    },
    Replace(NextScreen),
}

impl NextScreen {
    pub async fn show(self, cx: &mut TuiContext) -> Result<Navigation> {
        match self {
            NextScreen::LoadHomeScreen => load_home_screen(cx).await,
            NextScreen::HomeScreen(data) => display_home_screen(cx, data).await,
            NextScreen::PlayItem(media_item) => {
                crate::mpv::play(cx, media_item).await?;
                todo!()
            }
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
