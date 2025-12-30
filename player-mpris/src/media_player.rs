use player_core::{Command, PlayerHandle, state::SharedPlayerState};
use zbus::interface;

pub struct MediaPlayer2 {
    player: PlayerHandle,
    state: SharedPlayerState,
}

impl MediaPlayer2 {
    pub fn new(player: PlayerHandle, state: SharedPlayerState) -> Self {
        Self { player, state }
    }
}

#[interface(name = "org.mpris.MediaPlayer2", spawn = false)]
impl MediaPlayer2 {
    fn raise(&self) {}
    fn quit(&self) {
        self.player.send(Command::Stop);
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn fullscreen(&self) -> bool {
        self.state.lock().fullscreen
    }
    #[zbus(property)]
    fn set_fullscreen(&self, fullscreen: bool) {
        self.player.send(Command::Fullscreen(fullscreen));
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_set_fullscreen(&self) -> bool {
        true
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        true
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> &'static str {
        "Jellyfin TUI Player"
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn desktop_entry(&self) -> &'static str {
        "jellyhaj"
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> &'static [&'static str] {
        &[]
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> &'static [&'static str] {
        &[]
    }
}
