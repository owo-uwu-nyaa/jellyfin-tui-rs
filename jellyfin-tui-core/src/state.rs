use std::collections::HashMap;

use color_eyre::{Result, eyre::Report};
use entries::{image::available::ImagesAvailable, list::EntryList, screen::EntryScreen};
use jellyfin::{items::MediaItem, user_views::UserView};
use tracing::{debug, instrument};

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum LoadPlay {
    Movie(MediaItem),
    Series { id: String },
    Season { series_id: String, id: String },
    Episode { series_id: String, id: String },
    Playlist { id: String },
}

#[derive(Debug)]
pub enum NextScreen {
    LoadHomeScreen,
    HomeScreenData {
        resume: Vec<MediaItem>,
        next_up: Vec<MediaItem>,
        views: Vec<UserView>,
        latest: HashMap<String, Vec<MediaItem>>,
    },
    HomeScreen(EntryScreen, ImagesAvailable),
    LoadUserView(UserView),
    UserView {
        view: UserView,
        items: Vec<MediaItem>,
    },
    LoadPlayItem(LoadPlay),
    Play {
        items: Vec<MediaItem>,
        index: usize,
    },
    Error(Report),
    ItemDetails(MediaItem),
    ItemListDetailsData(MediaItem, Vec<MediaItem>),
    ItemListDetails(MediaItem, EntryList, ImagesAvailable),
    FetchItemListDetails(MediaItem),
    FetchItemListDetailsRef(String),
    FetchItemDetails(String),
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

#[derive(Debug)]
pub struct State {
    screen_stack: Vec<NextScreen>,
}

impl State {
    #[instrument(skip_all)]
    pub fn navigate(&mut self, nav: Navigation) {
        debug!("navigate instruction: {nav:?}");
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
    #[instrument(skip_all)]
    pub fn pop(&mut self) -> Option<NextScreen> {
        debug!("state stack: {:?}", self.screen_stack);
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

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ToNavigation {
    fn to_nav(self) -> Navigation;
}

impl ToNavigation for Result<Navigation> {
    fn to_nav(self) -> Navigation {
        match self {
            Ok(v) => v,
            Err(e) => Navigation::Replace(NextScreen::Error(e)),
        }
    }
}
