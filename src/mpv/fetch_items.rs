use std::pin::pin;

use color_eyre::{eyre::Context, Result};
use futures_util::StreamExt;
use jellyfin::{
    items::{ItemType, MediaItem},
    playlist::GetPlaylistItemsQuery,
    shows::GetEpisodesQuery,
    Auth, JellyfinClient, JellyfinVec,
};
use ratatui::widgets::{Block, Paragraph};
use tracing::warn;

use crate::{
    keybinds::{KeybindEvent, KeybindEventStream, LoadingCommand},
    state::{Navigation, NextScreen},
    TuiContext,
};

async fn fetch_items(
    cx: &JellyfinClient<Auth>,
    item: MediaItem,
) -> Result<(Vec<MediaItem>, usize)> {
    Ok(match item {
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            user_data: _,
            sort_name: _,
            item_type: ItemType::Series,
        } => (fetch_series(cx, &id).await?, 0),
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            user_data: _,
            sort_name: _,
            item_type:
                ItemType::Season {
                    series_id,
                    series_name: _,
                },
        } => {
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
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            user_data: _,
            item_type:
                ItemType::Episode {
                    container: _,
                    season_id: _,
                    season_name: _,
                    series_id,
                    series_name: _,
                    episode_index:_,
                    seasion_index:_,
                },
            sort_name: _,
        } => {
            let all = fetch_series(cx, &series_id).await?;
            let position = item_position(&id, &all);
            (all, position)
        }
        MediaItem {
            id,
            image_tags: _,
            media_type: _,
            name: _,
            sort_name: _,
            item_type: ItemType::Playlist,
            user_data: _,
        } => {
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

        item @ MediaItem {
            id: _,
            image_tags: _,
            media_type: _,
            name: _,
            user_data: _,
            sort_name: _,
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

pub async fn fetch_screen(cx: &mut TuiContext, item: MediaItem) -> Result<Navigation> {
    let msg = Paragraph::new("Loading related items for playlist")
        .centered()
        .block(Block::bordered());
    let mut fetch = pin!(fetch_items(&cx.jellyfin, item));
    let mut events = KeybindEventStream::new(&mut cx.events, cx.config.keybinds.fetch_mpv.clone());
    loop {
        cx.term
            .draw(|frame| {
                frame.render_widget(&msg, events.inner(frame.area()));
                frame.render_widget(&mut events, frame.area());
            })
            .context("rendering ui")?;
        tokio::select! {
            data = &mut fetch => {
                let (items,index )= data.context("loading home screen data")?;
                break Ok(Navigation::Replace(NextScreen::PlayItem { items , index  }))
            }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(LoadingCommand::Quit))) => break Ok(Navigation::PopContext),
                    Some(Ok(KeybindEvent::Render)) => continue,
                    Some(Ok(KeybindEvent::Text(_))) => unimplemented!(),
                    Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
                    None => break Ok(Navigation::Exit),
                }
            }
        }
    }
}
