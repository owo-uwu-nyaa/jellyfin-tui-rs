use std::pin::Pin;

use color_eyre::{eyre::Context, Result};
use jellyfin::{
    items::MediaItem, playlist::GetPlaylistItemsQuery, shows::GetEpisodesQuery, Auth,
    JellyfinClient, JellyfinVec,
};
use tracing::warn;

use crate::{
    state::{Navigation, NextScreen},
    TuiContext,
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum LoadPlay {
    Movie(MediaItem),
    Series { id: String },
    Season { series_id: String, id: String },
    Episode { series_id: String, id: String },
    Playlist { id: String },
}

async fn fetch_items(cx: &JellyfinClient<Auth>, item: LoadPlay) -> Result<(Vec<MediaItem>, usize)> {
    Ok(match item {
        LoadPlay::Series { id } => (fetch_series(cx, &id).await?, 0),
        LoadPlay::Season { series_id, id } => {
            let all = fetch_series(cx, &series_id).await?;
            let user_id = cx.get_auth().user.id.as_str();
            let season_items = cx
                .get_episodes(
                    &series_id,
                    &GetEpisodesQuery {
                        user_id: user_id.into(),
                        is_missing: false.into(),
                        start_index: 0.into(),
                        limit: 1.into(),
                        season_id: id.as_str().into(),
                        enable_images: false.into(),
                        enable_user_data: false.into(),
                        ..Default::default()
                    },
                )
                .await
                .context("fetching media items")?
                .deserialize()
                .await
                .context("deserializing media items")?
                .items;
            let position = if let Some(first) = season_items.first() {
                item_position(&first.id, &all)
            } else {
                warn!("no items found for season");
                0
            };
            (all, position)
        }
        LoadPlay::Episode { series_id, id } => {
            let all = fetch_series(cx, &series_id).await?;
            let position = item_position(&id, &all);
            (all, position)
        }
        LoadPlay::Playlist { id } => {
            let user_id = cx.get_auth().user.id.as_str();
            let items = JellyfinVec::collect(async |start| {
                cx.get_playlist_items(
                    &id,
                    &GetPlaylistItemsQuery {
                        user_id: user_id.into(),
                        start_index: start.into(),
                        limit: 100.into(),
                        enable_images: Some(true),
                        image_type_limit: 1.into(),
                        enable_image_types: "Primary, Backdrop, Thumb".into(),
                        enable_user_data: true.into(),
                    },
                )
                .await
                .context("fetching playlist items")?
                .deserialize()
                .await
                .context("deserializing playlist items")
            })
            .await?;
            (items, 0)
        }
        LoadPlay::Movie(item) => (vec![item], 0),
    })
}

fn item_position(id: &str, items: &[MediaItem]) -> usize {
    for (index, item) in items.iter().enumerate() {
        if item.id == id {
            return index;
        }
    }
    warn!("no such item found");
    0
}

async fn fetch_series(cx: &JellyfinClient<Auth>, series_id: &str) -> Result<Vec<MediaItem>> {
    let user_id = cx.get_auth().user.id.as_str();
    let res = JellyfinVec::collect(async |start| {
        cx.get_episodes(
            series_id,
            &GetEpisodesQuery {
                user_id: user_id.into(),
                is_missing: false.into(),
                start_index: start.into(),
                limit: 100.into(),
                enable_images: Some(true),
                image_type_limit: 1.into(),
                enable_image_types: "Primary, Backdrop, Thumb".into(),
                enable_user_data: true.into(),
                ..Default::default()
            },
        )
        .await
        .context("fetching media items")?
        .deserialize()
        .await
        .context("deserializing media items")
    })
    .await?;
    Ok(res)
}

pub async fn fetch_screen(cx: Pin<&mut TuiContext>, item: LoadPlay) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    crate::fetch::fetch_screen(
        "Loading related items for playlist",
        async {
            let (items, index) = fetch_items(jellyfin, item)
                .await
                .context("loading home screen data")?;
            Ok(Navigation::Replace(NextScreen::PlayItem { items, index }))
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}
