use color_eyre::{eyre::Context, Result};
use jellyfin::{
    items::{ItemType, MediaItem},
    shows::GetEpisodesQuery,
    JellyfinVec,
};
use tracing::{info, warn};

use crate::TuiContext;

pub async fn fetch_items(cx: &TuiContext, item: MediaItem) -> Result<(Vec<MediaItem>, usize)> {
    Ok(match item {
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            item_type: ItemType::Series,
        } => (fetch_series(cx, &id).await?, 0),
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            item_type:
                ItemType::Season {
                    series_id,
                    series_name: _,
                },
        } => {
            let all = fetch_series(cx, &series_id).await?;
            let user_id = cx.jellyfin.get_auth().user.id.as_str();
            let season_items = cx
                .jellyfin
                .get_episodes(
                    &series_id,
                    &GetEpisodesQuery {
                        user_id: user_id.into(),
                        is_missing: false.into(),
                        start_index: 0.into(),
                        limit: 1.into(),
                        season_id: id.as_str().into(),
                        enable_images: Some(true),
                        image_type_limit: Some(1),
                        enable_image_types: Some("Primary, Backdrop, Thumb"),
                        enable_user_data: Some(false),
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
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            item_type:
                ItemType::Episode {
                    container: _,
                    season_id: _,
                    season_name: _,
                    series_id,
                    series_name: _,
                },
        } => {
            let all = fetch_series(cx, &series_id).await?;
            let position = item_position(&id, &all);
            (all, position)
        }
        item @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            item_type: ItemType::Movie { container: _ },
        } => (vec![item], 0),
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

async fn fetch_series(cx: &TuiContext, series_id: &str) -> Result<Vec<MediaItem>> {
    let user_id = cx.jellyfin.get_auth().user.id.as_str();
    let res = JellyfinVec::collect(async |start| {
        cx.jellyfin
            .get_episodes(
                series_id,
                &GetEpisodesQuery {
                    user_id: user_id.into(),
                    is_missing: false.into(),
                    start_index: start.into(),
                    limit: 12.into(),
                    enable_images: Some(true),
                    image_type_limit: Some(1),
                    enable_image_types: Some("Primary, Backdrop, Thumb"),
                    enable_user_data: Some(false),
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
    info!("series: {res:#?}");
    todo!()
}
