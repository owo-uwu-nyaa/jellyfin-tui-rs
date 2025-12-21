use jellyfin::JellyfinClient;
use player_core::{Command, PlayerHandle, state::SharedPlayerState};
use tracing::error;
use zbus::{
    fdo::{Error, Result},
    interface,
    object_server::SignalEmitter,
    zvariant::ObjectPath,
};

use crate::types::{LoopStatus, Metadata, PlaybackStatus, parse_track_id};

pub struct Player {
    player: PlayerHandle,
    jellyfin: JellyfinClient,
    state: SharedPlayerState,
}

impl Player {
    pub fn new(player: PlayerHandle, jellyfin: JellyfinClient, state: SharedPlayerState) -> Self {
        Self {
            player,
            jellyfin,
            state,
        }
    }
}

pub fn pos_to_mpris(secs: f64) -> i64 {
    (secs * 1000000.0) as i64
}

#[interface(name = "org.mpris.MediaPlayer2.Player", spawn = false)]
impl Player {
    fn next(&self) {
        self.player.send(Command::Next);
    }
    fn previous(&self) {
        self.player.send(Command::Previous);
    }
    fn pause(&self) {
        self.player.send(Command::Pause(true));
    }
    fn play_pause(&self) {
        self.player.send(Command::TogglePause);
    }
    fn stop(&self) {
        self.player.send(Command::Stop);
    }
    fn play(&self) {
        self.player.send(Command::Pause(false));
    }
    fn seek(&self, micros: i64) {
        let secs = (micros as f64) / 1000000.0;
        self.player.send(Command::SeekRelative(secs));
    }
    #[zbus(name = "SetPosition")]
    fn set_playback_position(&self, track: ObjectPath<'_>, micros: i64) -> Result<()> {
        let track_id = parse_track_id(&track)?
            .ok_or_else(|| Error::InvalidArgs("Track id is NoTrack".to_owned()))?;
        self.player.send(Command::Play(track_id));
        self.player.send(Command::Seek((micros as f64) / 1000000.0));
        Ok(())
    }
    fn open_uri(&self, _uri: &str) -> Result<()> {
        Err(Error::NotSupported(
            "opening uri is not supported".to_string(),
        ))
    }

    #[zbus(signal)]
    pub async fn seeked(emitter: &SignalEmitter<'_>, pos: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> PlaybackStatus {
        let state = self.state.lock();
        if state.playlist.is_empty() {
            PlaybackStatus::Stopped
        } else if state.pause {
            PlaybackStatus::Paused
        } else {
            PlaybackStatus::Playing
        }
    }

    #[zbus(property)]
    fn loop_status(&self) -> LoopStatus {
        LoopStatus::None
    }

    #[zbus(property)]
    fn set_loop_status(&self, _l: LoopStatus) {}

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.state.lock().speed
    }

    #[zbus(property)]
    fn set_rate(&self, speed: f64) {
        if speed != 0.0 {
            self.player.send(Command::Speed(speed));
        } else {
            self.player.send(Command::Pause(true));
        }
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn set_shuffle(&self, _v: bool) {}

    #[zbus(property)]
    fn metadata(&self) -> Result<Metadata> {
        let state = self.state.lock();
        if let Some(current) = state.current {
            if let Some(item) = state.playlist.get(current) {
                Ok(Metadata::new(item, &self.jellyfin))
            } else {
                error!("unable to get playlist data for current item");
                Err(Error::Failed(
                    "unable to get playlist data for current item".to_owned(),
                ))
            }
        } else {
            Ok(Metadata::default())
        }
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        (self.state.lock().volume as f64) / 100.0
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        self.player.send(Command::Volume((volume * 100.0) as i64));
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        pos_to_mpris(self.state.lock().position)
    }
    #[zbus(property)]
    fn set_position(&self, pos: i64) {
        self.seek(pos);
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        0.1
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        5.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.can_play()
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.can_play()
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        !self.state.lock().stopped
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.can_play()
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.can_play()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}
