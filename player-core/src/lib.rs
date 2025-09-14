use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use color_eyre::Result;
use jellyfin::{JellyfinClient, items::MediaItem};
use libmpv::MpvProfile;
use tokio::sync::{mpsc, watch};

mod create;
mod log;
mod mpv_stream;
mod poll;

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Close,
    Play,
    Pause,
    PlayPause,
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub index: usize,
    pub current: Arc<MediaItem>,
    pub pause: bool,
    pub position: f64,
}

pub struct PlayerHandle {
    inner: PlayerRef,
}

impl Debug for PlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerHandle")
            .field("closed", &self.inner.closed.load(Ordering::Relaxed))
            .field("state", &self.inner.state.borrow().deref())
            .finish()
    }
}

impl PlayerHandle {
    pub fn new(
        jellyfin: &JellyfinClient,
        hwdec: &str,
        profile: MpvProfile,
        log_level: &str,
        items: Vec<MediaItem>,
        index: usize,
    ) -> Result<Self> {
        create::player(jellyfin, hwdec, profile, log_level, items, index)
    }
    pub fn new_ref(&self) -> PlayerRef {
        self.inner.clone()
    }
}

impl Deref for PlayerHandle {
    type Target = PlayerRef;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PlayerHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for PlayerHandle {
    fn drop(&mut self) {
        self.send(Command::Close);
    }
}

#[derive(Clone)]
pub struct PlayerRef {
    closed: Arc<AtomicBool>,
    send: mpsc::UnboundedSender<Command>,
    state: watch::Receiver<PlayerState>,
}

impl PlayerRef {
    pub fn send(&self, command: Command) {
        if !self.closed.load(Ordering::Relaxed) && self.send.send(command).is_err() {
            self.closed.store(true, Ordering::Relaxed);
        };
    }
    pub fn state(&self) -> &watch::Receiver<PlayerState> {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut watch::Receiver<PlayerState> {
        &mut self.state
    }
}

impl Debug for PlayerRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerRef")
            .field("closed", &self.closed.load(Ordering::Relaxed))
            .field("state", &self.state.borrow().deref())
            .finish()
    }
}
