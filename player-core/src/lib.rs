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
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::DropGuard;

use crate::state::EventReceiver;

mod create;
mod log;
mod mpv_stream;
mod poll;
pub mod state;

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
    pub id: u64,
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

#[derive(Debug)]
pub enum Command {
    Pause(bool),
    TogglePause,
    Fullscreen(bool),
    Minimized(bool),
    Next,
    Previous,
    Seek(f64),
    SeekRelative(f64),
    Speed(f64),
    Volume(i64),
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
    GetEventReceiver(oneshot::Sender<EventReceiver>),
}

type Playlist = Arc<Vec<Arc<PlaylistItem>>>;

#[derive(Debug, Clone)]
pub enum Events {
    ReplacePlaylist {
        current: Option<PlaylistItemId>,
        current_index: Option<usize>,
        new_playlist: Playlist,
    },
    AddPlaylistItem {
        after: Option<PlaylistItemId>,
        index: usize,
        new_playlist: Playlist,
    },
    RemovePlaylistItem {
        removed: PlaylistItemId,
        new_playlist: Playlist,
    },
    Current(Option<usize>),
    Paused(bool),
    Stopped(bool),
    Position(f64),
    Seek(f64),
    Speed(f64),
    Fullscreen(bool),
    Volume(i64),
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub playlist: Arc<Vec<Arc<PlaylistItem>>>,
    pub current: Option<usize>,
    pub pause: bool,
    pub stopped: bool,
    pub position: f64,
    pub speed: f64,
    pub fullscreen: bool,
    pub volume: i64,
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
}

pub struct OwnedPlayerHandle {
    inner: PlayerHandle,
    _stop: DropGuard,
}

impl Deref for OwnedPlayerHandle {
    type Target = PlayerHandle;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl PlayerHandle {
    pub fn send(&self, command: Command) {
        if !self.closed.load(Ordering::Relaxed) && self.send.send(command).is_err() {
            self.closed.store(true, Ordering::Relaxed);
        };
    }
    pub async fn get_state(&self) -> Result<EventReceiver, oneshot::error::RecvError> {
        let (send, receive) = oneshot::channel();
        self.send(Command::GetEventReceiver(send));
        receive.await
    }
}

impl Debug for PlayerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerRef")
            .field("closed", &self.closed.load(Ordering::Relaxed))
            .finish()
    }
}
