use entries::entry::{Entry, EntryInner};
use jellyfin::items::{ItemType, MediaItem};

use crate::state::{LoadPlay, NextScreen};

pub trait EntryExt {
    fn item_id(&self) -> Option<&str>;
    fn play(&self) -> Option<NextScreen>;
    fn open(&self) -> NextScreen;
    fn play_open(&self) -> NextScreen;
    fn episode(&self) -> Option<NextScreen>;
    fn season(&self) -> Option<NextScreen>;
    fn series(&self) -> Option<NextScreen>;
}

impl EntryExt for Entry {
    fn item_id(&self) -> Option<&str> {
        match self.inner() {
            EntryInner::Item(media_item) => Some(media_item.id.as_str()),
            EntryInner::View(_) => None,
        }
    }
    fn play(&self) -> Option<NextScreen> {
        match self.inner() {
            EntryInner::View(_) => None,
            EntryInner::Item(item) => Some(play(item)),
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
            EntryInner::Item(item) => play(item),
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
pub fn play(item: &MediaItem) -> NextScreen {
    NextScreen::LoadPlayItem(match item {
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
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Music { album_id, album: _ },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::Music {
            id: id.clone(),
            album_id: album_id.clone(),
        },
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::MusicAlbum,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => LoadPlay::MusicAlbum { id: id.clone() },
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Unknown | ItemType::CollectionFolder,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => return NextScreen::UnsupportedItem,
    })
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
                | ItemType::Music {
                    album_id: _,
                    album: _,
                }
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
                | ItemType::MusicAlbum
                | ItemType::CollectionFolder
                | ItemType::Season {
                    series_id: _,
                    series_name: _,
                },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::FetchItemListDetails(v.clone()),
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Unknown,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::UnsupportedItem,
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
                | ItemType::Music {
                    album_id: _,
                    album: _,
                }
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
                | ItemType::MusicAlbum
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
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Unknown | ItemType::Folder | ItemType::CollectionFolder,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => NextScreen::UnsupportedItem,
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
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Music { album_id, album: _ },
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetailsRef(album_id.clone())),
        i @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::MusicAlbum,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
        MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            overview: _,
            item_type: ItemType::Unknown | ItemType::CollectionFolder,
            user_data: _,
            episode_index: _,
            season_index: _,
            run_time_ticks: _,
        } => Some(NextScreen::UnsupportedItem),
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
