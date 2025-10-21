use std::{mem, sync::Arc};

use color_eyre::eyre::Context;
use jellyfin::{JellyfinClient, playback_status::ProgressBody};
use player_core::PlayerHandle;
use spawn::spawn_res;
use tracing::{error_span, instrument};

fn send_playing(id: Arc<String>, jellyfin: &JellyfinClient) {
    let span = error_span!("send_playing");
    let started = jellyfin.prepare_set_playing();
    spawn_res(
        async move {
            started
                .context("Preparing start playback request")?
                .send(&id)
                .await
                .context("Sending start playback request")
        },
        span,
    );
}

fn send_progress(id: Arc<String>, position: f64, paused: bool, jellyfin: &JellyfinClient) {
    let span = error_span!("send_progress");
    let finished = jellyfin.prepare_set_playing_progress();
    spawn_res(
        async move {
            finished
                .context("Preparing playback progress request")?
                .send(&ProgressBody {
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

fn send_playing_stopped(id: String, position: f64, jellyfin: &JellyfinClient) {
    let span = error_span!("send_playing_stopped");
    let finished = jellyfin.prepare_set_playing_stopped();
    spawn_res(
        async move {
            finished?
                .send(&ProgressBody {
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
            send_playing(id.clone(), &jellyfin);
        }
        (state.current, id, state.position)
    };
    loop {
        if player.state_mut().changed().await.is_err() {
            if let Some(id) = old_id.as_mut() {
                send_playing_stopped(mem::take(Arc::make_mut(id)), old_position, &jellyfin);
            }
            break;
        } else {
            let state = player.state().borrow();
            if current != state.current {
                if let Some(index) = state.current {
                    let new_id = if let Some(old_id) = old_id.as_mut() {
                        let old = mem::replace(
                            Arc::make_mut(old_id),
                            state.playlist[index].item.id.clone(),
                        );
                        send_playing_stopped(old, old_position, &jellyfin);
                        old_id.clone()
                    } else {
                        let new = Arc::new(state.playlist[index].item.id.clone());
                        old_id = Some(new.clone());
                        new
                    };
                    send_playing(new_id, &jellyfin);
                } else if let Some(old_id) = old_id.as_mut() {
                    send_playing_stopped(mem::take(Arc::make_mut(old_id)), old_position, &jellyfin);
                }
                current = state.current;
                send_tick = 11;
            } else if send_tick == 0 {
                if let Some(old_id) = old_id.as_ref() {
                    send_progress(old_id.clone(), state.position, state.pause, &jellyfin);
                }
                send_tick = 11;
            }
            old_position = state.position;
            send_tick = send_tick.saturating_sub(1)
        }
    }
}
