use player_core::{Command, PlayerRef};
use zbus::{interface, object_server::SignalEmitter, zvariant::ObjectPath};

struct MediaPlayer2 {
    player: PlayerRef,
}

#[interface(name = "org.mpris.MediaPlayer2", spawn = false)]
impl MediaPlayer2 {
    async fn quit(&self) {
        self.player.send(Command::Close);
    }
    async fn raise(&self) -> () {}
    #[zbus(property(emits_changed_signal = "const"))]
    async fn can_quit(&self) -> bool {
        true
    }
    #[zbus(property)]
    async fn fullscreen(&self) -> bool {
        self.player.state().borrow().fullscreen
    }
    #[zbus(property)]
    async fn set_fullscreen(&self, fullscreen: bool) {
        self.player.send(if fullscreen {
            Command::SetFullscreen
        } else {
            Command::UnsetFullscreen
        });
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn can_set_fullscreen(&self) -> bool {
        true
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn identity(&self) -> &'static str {
        "Jellyfin TUI Player"
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn desktop_entry(&self) -> &'static str {
        "jellyfin-tui-rs"
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn supported_uri_schemes(&self) -> &'static [&'static str] {
        &[]
    }
}

struct Player {
    player: PlayerRef,
}

#[interface(name = "org.mpris.MediaPlayer2.Player", spawn = false)]
impl Player {
    async fn next(&self) {
        self.player.send(Command::Next);
    }
    async fn previous(&self) {
        self.player.send(Command::Previous);
    }
    async fn pause(&self) {
        self.player.send(Command::Pause);
    }
    async fn play_pause(&self) {
        self.player.send(Command::PlayPause);
    }
    async fn stop(&self) {
        self.player.send(Command::Pause);
        self.player.send(Command::Seek(0.0));
    }
    async fn play(&self) {
        self.player.send(Command::Play);
    }
    async fn seek(&self, offset: f64) {
        self.player.send(Command::Seek(offset / 1000000.0));
    }
    async fn set_position(&self, track_id: ObjectPath<'_>, offset: f64) {
        let _ = (track_id, offset);
    }
    async fn open_uri(&self, uri: &str) {
        let _ = uri;
    }
    async fn playback_status(&self) -> &'static str {
        if self.player.state().borrow().pause {
            "Paused"
        } else {
            "Playing"
        }
    }
    async fn loop_status(&self) -> &'static str {
        "None"
    }
    async fn set_loop_status(&self, _: &str) {}
    async fn rate(&self) -> f64 {
        1.0
    }
    async fn set_rate(&self, _: f64) {}
    async fn shuffle(&self) -> bool {
        false
    }
    async fn set_shuffle(&self, _: bool) {}

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: f64) -> zbus::Result<()>;
}

pub async fn run_mpris_service(mut handle: PlayerRef) -> color_eyre::Result<()> {
    let mp2 = MediaPlayer2 {
        player: handle.clone(),
    };
    let con = zbus::connection::Builder::session()?
        .name(format!(
            "org.mpris.MediaPlayer2.jellyfin_tui_rs.i{}",
            std::process::id()
        ))?
        .serve_at("/org/mpris/MediaPlayer2", mp2)?
        .build()
        .await?;
    while handle.state_mut().changed().await.is_ok() {}
    Ok(())
}
