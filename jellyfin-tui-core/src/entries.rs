use entries::entry::{Entry, EntryInner};
use jellyfin::items::{ItemType, MediaItem};

use crate::state::{LoadPlay, NextScreen};

pub trait EntryExt {
    fn play(&self) -> Option<NextScreen>;
    fn open(&self) -> NextScreen;
    fn play_open(&self) -> NextScreen;
    fn episode(&self) -> Option<NextScreen>;
    fn season(&self) -> Option<NextScreen>;
    fn series(&self) -> Option<NextScreen>;
}

impl EntryExt for Entry {
    fn play(&self) -> Option<NextScreen> {
        match self.inner() {
            EntryInner::View(_) => None,
            EntryInner::Item(item) => Some(NextScreen::LoadPlayItem(play(item))),
        }
    }
    fn open(&self) -> NextScreen {
        match self.inner() {
            EntryInner::View(view) => NextScreen::LoadUserView(view.clone()),
            EntryInner::Item(item) => open(item),
        }
    }
    fn play_open(&self) -> NextScreen {
        match self.inner() {
            EntryInner::View(view) => NextScreen::LoadUserView(view.clone()),
            EntryInner::Item(item) => NextScreen::LoadPlayItem(play(item)),
        }
    }
    fn episode(&self) -> Option<NextScreen> {
        match self.inner() {
            EntryInner::Item(i) => Some(episode(i)),
            _ => None,
        }
    }
    fn season(&self) -> Option<NextScreen> {
        match self.inner() {
            EntryInner::Item(i) => season(i),
            _ => None,
        }
    }
    fn series(&self) -> Option<NextScreen> {
        match self.inner() {
            EntryInner::Item(i) => series(i),
            _ => None,
        }
    }
}
pub fn play(item: &MediaItem) -> LoadPlay {
    match item {
        v @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Movie,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Movie(v.clone()),
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Playlist | ItemType::Folder,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Playlist { id: id.clone() },
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Series,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Series { id: id.clone() },
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Season {
                    series_id,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Season {
            series_id: series_id.clone(),
            id: id.clone(),
        },
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Episode {
                    season_id: _,
                    season_name: _,
                    series_id,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Episode {
            series_id: series_id.clone(),
            id: id.clone(),
        },
    }
}

fn open(item: &MediaItem) -> NextScreen {
    match item {
        v @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Movie
                | ItemType::Episode {
                    season_id: _,
                    season_name: _,
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::ItemDetails(v.clone()),
        v @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Playlist
                | ItemType::Folder
                | ItemType::Series
                | ItemType::Season {
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::FetchItemListDetails(v.clone()),
    }
}
fn episode(item: &MediaItem) -> NextScreen {
    match item {
        v @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Movie
                | ItemType::Episode {
                    season_id: _,
                    season_name: _,
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::ItemDetails(v.clone()),
        i @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Playlist
                | ItemType::Folder
                | ItemType::Series
                | ItemType::Season {
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::ItemDetails(i.clone()),
    }
}

pub fn season(item: &MediaItem) -> Option<NextScreen> {
    match item {
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Episode {
                    season_id: Some(id),
                    season_name: _,
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetailsRef(id.clone())),
        i @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Season {
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
        i @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Series,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
        _ => None,
    }
}

fn series(item: &MediaItem) -> Option<NextScreen> {
    match item {
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type:
                ItemType::Episode {
                    season_id: _,
                    season_name: _,
                    series_id,
                    series_name: _,
                }
                | ItemType::Season {
                    series_id,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetailsRef(series_id.clone())),
        i @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Series,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
        _ => None,
    }
}
