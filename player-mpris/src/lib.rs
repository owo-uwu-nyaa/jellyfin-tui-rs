mod media_player;
mod player;
mod track_list;
mod types;

use std::{borrow::Cow, collections::HashMap};

use color_eyre::eyre::{Context, OptionExt, Result, eyre};
use jellyfin::JellyfinClient;
use player_core::PlayerHandle;
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};
use zbus::{
    fdo::Properties,
    names::InterfaceName,
    object_server::{Interface, SignalEmitter},
    zvariant::Value,
};

trait TraceError {
    fn trace_error(self);
}
impl TraceError for Result<()> {
    fn trace_error(self) {
        if let Err(e) = self {
            warn!("error in mpris interface: {e:?}")
        }
    }
}

use crate::{
    media_player::MediaPlayer2,
    player::{Player, pos_to_mpris},
    track_list::TrackList,
    types::{Metadata, PlaybackStatus, track_id_as_object},
};

const MPRIS: &str = "/org/mpris/MediaPlayer2";

async fn property_changed<I: Interface>(
    emitter: &SignalEmitter<'_>,
    name: &'static str,
    val: impl Into<Value<'_>>,
) {
    async fn inner(
        emitter: &SignalEmitter<'_>,
        inter_name: InterfaceName<'static>,
        name: &'static str,
        val: Value<'_>,
    ) {
        let mut changed = HashMap::with_capacity(1);
        changed.insert(name, val);
        Properties::properties_changed(emitter, inter_name.as_ref(), changed, Cow::Borrowed(&[]))
            .await
            .with_context(|| {
                format!("sending property changed for {name} in interface {inter_name}")
            })
            .trace_error();
    }
    inner(emitter, I::name(), name, val.into()).await
}

pub async fn run_mpris_service(
    handle: PlayerHandle,
    jellyfin: JellyfinClient,
) -> color_eyre::Result<()> {
    let mut state = handle
        .get_state()
        .await
        .map_err(|_| eyre!("mpv handle is already closed"))?
        .with_shared_state();
    let mp2 = MediaPlayer2::new(handle.clone(), state.clone());
    let p = Player::new(handle.clone(), jellyfin.clone(), state.clone());
    let t = TrackList::new(handle.clone(), jellyfin.clone(), state.clone());
    let conn = zbus::connection::Builder::session()?
        .name(format!(
            "org.mpris.MediaPlayer2.jellyfin_tui_rs.i{}",
            std::process::id()
        ))?
        .serve_at(MPRIS, mp2)?
        .serve_at(MPRIS, p)?
        .serve_at(MPRIS, t)?
        .build()
        .await?;
    let emitter = SignalEmitter::new(&conn, MPRIS).context("getting signal emitter")?;
    loop {
        match state
            .receive_inspect(async |event, state| -> Result<()> {
                match event {
                    player_core::Events::ReplacePlaylist {
                        current,
                        current_index: _,
                        new_playlist,
                    } => {
                        let ids: Vec<_> = new_playlist
                            .iter()
                            .map(|i| track_id_as_object(Some(i.id)))
                            .collect();
                        TrackList::track_list_replaced(
                            &emitter,
                            ids,
                            track_id_as_object(*current).as_ref(),
                        )
                        .await
                        .context("emitting TrackListReplaced")
                        .trace_error();
                        invalidate_tracks(&emitter).await;
                    }
                    player_core::Events::AddPlaylistItem {
                        after,
                        index,
                        new_playlist,
                    } => {
                        let metadata = Metadata::new(&new_playlist[*index], &jellyfin);
                        TrackList::track_added(
                            &emitter,
                            metadata,
                            track_id_as_object(*after).as_ref(),
                        )
                        .await
                        .context("emitting TrackAdded")
                        .trace_error();
                        invalidate_tracks(&emitter).await;
                    }
                    player_core::Events::RemovePlaylistItem {
                        removed,
                        new_playlist: _,
                    } => {
                        TrackList::track_removed(
                            &emitter,
                            track_id_as_object(Some(*removed)).as_ref(),
                        )
                        .await
                        .context("emitting TrackRemoved")
                        .trace_error();

                        invalidate_tracks(&emitter).await;
                    }
                    player_core::Events::Current(None) => {
                        property_changed::<Player>(&emitter, "Metadata", Metadata::default()).await
                    }
                    player_core::Events::Current(Some(pos)) => {
                        let metadata = Metadata::new(
                            state
                                .lock()
                                .playlist
                                .get(*pos)
                                .ok_or_eyre("current index is not in playlist")?,
                            &jellyfin,
                        );
                        property_changed::<Player>(&emitter, "Metadata", metadata).await
                    }
                    player_core::Events::Paused(paused) => {
                        if !state.lock().stopped {
                            let status = if *paused {
                                PlaybackStatus::Paused
                            } else {
                                PlaybackStatus::Playing
                            };
                            property_changed::<Player>(&emitter, "PlaybackStatus", status).await
                        }
                    }
                    player_core::Events::Stopped(stopped) => {
                        let val = !stopped;
                        let paused = state.lock().pause;
                        let mut changed = HashMap::with_capacity(6);
                        changed.insert("CanGoNext", val.into());
                        changed.insert("CanGoPrevious", val.into());
                        changed.insert("CanPlay", val.into());
                        changed.insert("CanPause", val.into());
                        changed.insert("CanSeek", val.into());
                        changed.insert(
                            "PlaybackStatus",
                            match (stopped, paused) {
                                (true, _) => PlaybackStatus::Stopped,
                                (false, true) => PlaybackStatus::Paused,
                                (false, false) => PlaybackStatus::Playing,
                            }
                            .into(),
                        );
                        Properties::properties_changed(
                            &emitter,
                            <Player>::name(),
                            changed,
                            Cow::Borrowed(&[]),
                        )
                        .await
                        .context("sending property changed in interface Player")
                        .trace_error();
                    }
                    player_core::Events::Position(_) => {}
                    player_core::Events::Seek(pos) => Player::seeked(&emitter, pos_to_mpris(*pos))
                        .await
                        .context("sending seek signal")
                        .trace_error(),
                    player_core::Events::Speed(speed) => {
                        property_changed::<Player>(&emitter, "Rate", speed).await
                    }
                    player_core::Events::Fullscreen(f) => {
                        property_changed::<MediaPlayer2>(&emitter, "Fullscreen", f).await
                    }
                    player_core::Events::Volume(vol) => {
                        property_changed::<Player>(&emitter, "Volume", (*vol as f64) / 100.0).await
                    }
                }
                Ok(())
            })
            .await
        {
            Ok(res) => res?,
            Err(RecvError::Closed) => {
                info!("mpris player closed");
                break;
            }
            Err(RecvError::Lagged(_)) => {
                warn!("lagged while processing events, data might be unreliable");
            }
        }
    }

    Ok(())
}

async fn invalidate_tracks(emitter: &SignalEmitter<'_>) {
    Properties::properties_changed(
        emitter,
        TrackList::name(),
        HashMap::new(),
        Cow::Borrowed(&["Tracks"]),
    )
    .await
    .context("invalidating Tracks on TrackList")
    .trace_error();
}
