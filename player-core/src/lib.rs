use std::{
    fmt::{Debug, Display},
    num::ParseIntError,
    ops::Deref,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use jellyfin::items::MediaItem;
use tokio::sync::{mpsc, watch};

mod create;
pub mod diff;
mod log;
mod mpv_stream;
mod poll;

#[derive(Debug, Default)]
pub struct PlaylistItemIdGen {
    id: u64,
}

impl PlaylistItemIdGen {
    fn next(&mut self) -> PlaylistItemId {
        let r = self.id;
        self.id = self.id.wrapping_add(1);
        PlaylistItemId { id: r }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaylistItemId {
    pub(crate) id: u64,
}

impl Display for PlaylistItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.id, f)
    }
}

impl FromStr for PlaylistItemId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(PlaylistItemId {
            id: FromStr::from_str(s)?,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Pause(bool),
    Fullscreen(bool),
    Minimized(bool),
    Next,
    Previous,
    Seek(f64),
    Play(PlaylistItemId),
    AddTrack {
        item: Box<MediaItem>,
        after: Option<PlaylistItemId>,
        play: bool,
    },
    Remove(PlaylistItemId),
    ReplacePlaylist {
        items: Vec<MediaItem>,
        first: usize,
    },
    Stop,
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub playlist: Arc<Vec<Arc<PlaylistItem>>>,
    pub current: Option<usize>,
    pub pause: bool,
    pub idle: bool,
    pub position: f64,
    pub fullscreen: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone)]
pub struct PlaylistItem {
    pub item: MediaItem,
    pub id: PlaylistItemId,
}

#[derive(Clone)]
pub struct PlayerHandle {
    closed: Arc<AtomicBool>,
    send: mpsc::UnboundedSender<Command>,
    state: watch::Receiver<PlayerState>,
}

impl PlayerHandle {
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

impl Debug for PlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerRef")
            .field("closed", &self.closed.load(Ordering::Relaxed))
            .field("state", &self.state.borrow().deref())
            .finish()
    }
}
