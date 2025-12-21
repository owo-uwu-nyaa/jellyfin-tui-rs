use std::{ops::Deref, sync::Arc};

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::{Events, PlayerState};

pub trait State {
    fn update(&mut self, event: Events);
}

impl State for PlayerState {
    fn update(&mut self, event: Events) {
        match event {
            Events::ReplacePlaylist {
                current: _,
                current_index,
                new_playlist,
            } => {
                self.current = current_index;
                self.playlist = new_playlist
            }
            Events::AddPlaylistItem {
                after: _,
                index: _,
                new_playlist,
            } => self.playlist = new_playlist,
            Events::RemovePlaylistItem {
                removed: _,
                new_playlist,
            } => self.playlist = new_playlist,
            Events::Current(c) => self.current = c,
            Events::Paused(p) => self.pause = p,
            Events::Stopped(s) => self.stopped = s,
            Events::Position(p) => self.position = p,
            Events::Seek(s) => self.position = s,
            Events::Speed(s) => self.speed = s,
            Events::Fullscreen(f) => self.fullscreen = f,
            Events::Volume(v) => self.volume = v,
        }
    }
}

pub type SharedPlayerState = Arc<Mutex<PlayerState>>;

impl State for SharedPlayerState {
    fn update(&mut self, event: Events) {
        self.lock().update(event);
    }
}

#[derive(Debug)]
pub struct EventReceiver<S: State = PlayerState> {
    pub(crate) state: S,
    pub(crate) receive: broadcast::Receiver<Events>,
}

impl<S: State> EventReceiver<S> {
    pub async fn receive_inspect<T>(
        &mut self,
        f: impl AsyncFnOnce(&Events, &S) -> T,
    ) -> Result<T, broadcast::error::RecvError> {
        let event = self.receive.recv().await?;
        let res = f(&event, &self.state).await;
        self.state.update(event);
        Ok(res)
    }
    pub async fn receive(&mut self) -> Result<(), broadcast::error::RecvError> {
        self.receive_inspect(async |_, _| {}).await
    }
}

impl<S: State> Deref for EventReceiver<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl EventReceiver<PlayerState> {
    pub fn with_shared_state(self) -> EventReceiver<Arc<Mutex<PlayerState>>> {
        EventReceiver {
            state: Arc::new(Mutex::new(self.state)),
            receive: self.receive,
        }
    }
}
