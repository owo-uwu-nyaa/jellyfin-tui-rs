use std::{mem, sync::Arc};

use color_eyre::eyre::Context;
use jellyfin::{JellyfinClient, playback_status::ProgressBody};
use player_core::PlayerHandle;
use spawn::spawn_res;
use tracing::{error_span, instrument};

fn send_playing(id: Arc<String>, jellyfin: JellyfinClient) {
    let span = error_span!("send_playing");
    spawn_res(
        async move {
            jellyfin
                .set_playing(&id)
                .await
                .context("Sending start playback request")
        },
        span,
    );
}

fn send_progress(id: Arc<String>, position: f64, paused: bool, jellyfin: JellyfinClient) {
    let span = error_span!("send_progress");
    spawn_res(
        async move {
            jellyfin
                .set_playing_progress(&ProgressBody {
                    item_id: &id,
                    position_ticks: (position * 10000000.0) as u64,
                    is_paused: paused,
                })
                .await
                .context("Sending playback progress request")
        },
        span,
    );
}

fn send_playing_stopped(id: Arc<String>, position: f64, jellyfin: JellyfinClient) {
    let span = error_span!("send_playing_stopped");
    spawn_res(
        async move {
            jellyfin
                .set_playing_stopped(&ProgressBody {
                    item_id: &id,
                    position_ticks: (position * 10000000.0) as u64,
                    is_paused: true,
                })
                .await
        },
        span,
    );
}

#[instrument(skip_all)]
pub async fn player_jellyfin(mut player: PlayerHandle, jellyfin: JellyfinClient) {
    let mut send_tick = 10u8;
    let (mut current, mut old_id, mut old_position) = {
        let state = player.state().borrow();
        let id = state
            .current
            .map(|i| Arc::new(state.playlist[i].item.id.clone()));
        if let Some(id) = id.as_ref() {
            send_playing(id.clone(), jellyfin.clone());
        }
        (state.current, id, state.position)
    };
    loop {
        if player.state_mut().changed().await.is_err() {
            if let Some(id) = old_id.as_mut() {
                send_playing_stopped(id.clone(), old_position, jellyfin.clone());
            }
            break;
        } else {
            let state = player.state().borrow();
            if current != state.current {
                if let Some(index) = state.current {
                    let new_id = if let Some(old_id) = old_id.as_mut() {
                        let new_id = Arc::new(state.playlist[index].item.id.clone());
                        let old = mem::replace(old_id, new_id.clone());
                        send_playing_stopped(old, old_position, jellyfin.clone());
                        new_id
                    } else {
                        let new = Arc::new(state.playlist[index].item.id.clone());
                        old_id = Some(new.clone());
                        new
                    };
                    send_playing(new_id, jellyfin.clone());
                } else if let Some(old_id) = old_id.take() {
                    send_playing_stopped(old_id, old_position, jellyfin.clone());
                }
                current = state.current;
                send_tick = 11;
            } else if send_tick == 0 {
                if let Some(old_id) = old_id.as_ref() {
                    send_progress(
                        old_id.clone(),
                        state.position,
                        state.pause,
                        jellyfin.clone(),
                    );
                }
                send_tick = 11;
            }
            old_position = state.position;
            send_tick = send_tick.saturating_sub(1)
        }
    }
}
