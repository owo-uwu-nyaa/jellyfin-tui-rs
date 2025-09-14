use std::{mem, sync::Arc};

use color_eyre::eyre::Context;
use jellyfin::{JellyfinClient, playback_status::ProgressBody};
use player_core::PlayerRef;
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

fn send_playing_stopped(id: Arc<String>, position: f64, jellyfin: &JellyfinClient) {
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
pub async fn player_jellyfin(mut player: PlayerRef, jellyfin: JellyfinClient) {
    let (mut current, mut old_id, mut old_position) = {
        let state = player.state().borrow();
        let id = Arc::new(state.current.id.clone());
        send_playing(id.clone(), &jellyfin);
        (state.index, id, state.position)
    };
    loop {
        let res = player.state_mut().changed().await;
        match res {
            Err(_) => {
                send_playing_stopped(old_id, old_position, &jellyfin);
                break;
            }
            Ok(()) => {
                let state = player.state().borrow();
                if current != state.index {
                    let old = mem::replace(&mut old_id, Arc::new(state.current.id.clone()));
                    send_playing_stopped(old, old_position, &jellyfin);
                    current = state.index;
                    send_playing(old_id.clone(), &jellyfin);
                } else {
                    send_progress(old_id.clone(), state.position, state.pause, &jellyfin);
                }
                old_position = state.position
            }
        }
    }
}
