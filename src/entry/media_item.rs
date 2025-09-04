use jellyfin::items::{ItemType, MediaItem};

use crate::{mpv::fetch_items::LoadPlay, state::NextScreen};

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
        } => LoadPlay::Episode {
            series_id: series_id.clone(),
            id: id.clone(),
        },
    }
}

pub fn open(item: &MediaItem) -> NextScreen {
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
        } => NextScreen::FetchItemListDetails(v.clone()),
    }
}
pub fn episode(item: &MediaItem) -> NextScreen {
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
        } => NextScreen::ItemDetails(v.clone()),
        MediaItem {
            id,
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
        } => NextScreen::FetchEpisodeDetails(id.clone()),
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
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
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
        } => Some(NextScreen::FetchSeasonDetailsRef(id.clone())),
        _ => None,
    }
}

pub fn series(item: &MediaItem) -> Option<NextScreen> {
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
        } => Some(NextScreen::FetchItemListDetails(i.clone())),
        _ => None,
    }
}
