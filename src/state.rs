use std::pin::Pin;

use jellyfin::{items::MediaItem, user_views::UserView};
use tracing::{debug, instrument};

use crate::{
    TuiContext,
    error::display_error,
    home_screen::{
        display_home_screen, handle_home_screen_data,
        load::{HomeScreenData, load_home_screen},
    },
    item_details::{display_fetch_episode, display_item_details},
    item_list_details::{
        display_fetch_item_list, display_fetch_item_list_ref, display_fetch_season,
        display_item_list_details, handle_item_list_details_data,
    },
    list::EntryList,
    mpv::{
        self,
        fetch_items::{LoadPlay, fetch_screen},
    },
    screen::EntryScreen,
    user_view::{display_user_view, fetch_user_view},
};
use color_eyre::eyre::{Report, Result};

#[derive(Debug)]
pub enum NextScreen {
    LoadHomeScreen,
    HomeScreenData(HomeScreenData),
    HomeScreen(EntryScreen),
    LoadUserView(UserView),
    UserView {
        view: UserView,
        items: Vec<MediaItem>,
    },
    LoadPlayItem(LoadPlay),
    PlayItem {
        items: Vec<MediaItem>,
        index: usize,
    },
    Error(Report),
    ItemDetails(MediaItem),
    ItemListDetailsData(MediaItem, Vec<MediaItem>),
    ItemListDetails(MediaItem, EntryList),
    FetchItemListDetails(MediaItem),
    FetchItemListDetailsRef(String),
    FetchEpisodeDetails(String),
    FetchSeasonDetailsRef(String),
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
    pub async fn show(self, cx: Pin<&mut TuiContext>) -> Result<Navigation> {
        match self {
            NextScreen::LoadHomeScreen => load_home_screen(cx).await,
            NextScreen::HomeScreenData(data) => handle_home_screen_data(cx, data).await,
            NextScreen::HomeScreen(screen) => display_home_screen(cx, screen).await,
            NextScreen::LoadUserView(view) => fetch_user_view(cx, view).await,
            NextScreen::UserView { view, items } => display_user_view(cx, view, items).await,
            NextScreen::LoadPlayItem(media_item) => fetch_screen(cx, media_item).await,
            NextScreen::PlayItem { items, index } => mpv::play(cx, items, index).await,
            NextScreen::Error(msg) => display_error(cx, msg).await,
            NextScreen::FetchItemListDetails(item) => display_fetch_item_list(cx, item).await,
            NextScreen::FetchItemListDetailsRef(item) => {
                display_fetch_item_list_ref(cx, &item).await
            }
            NextScreen::ItemListDetailsData(item, children) => {
                handle_item_list_details_data(cx, item, children).await
            }
            NextScreen::ItemListDetails(item, children) => {
                display_item_list_details(cx, item, children).await
            }
            NextScreen::FetchSeasonDetailsRef(series) => display_fetch_season(cx, &series).await,
            NextScreen::FetchEpisodeDetails(id) => display_fetch_episode(cx, &id).await,
            NextScreen::ItemDetails(episode) => display_item_details(cx, episode).await,
        }
    }
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
