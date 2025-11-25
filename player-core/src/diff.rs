use std::sync::Arc;

use crate::{PlayerState, PlaylistItem};

#[derive(Debug)]
pub struct PlayerStateChanged {
    pub playlist: Option<Arc<Vec<Arc<PlaylistItem>>>>,
    pub current: Option<Option<usize>>,
    pub pause: Option<bool>,
    pub position: Option<f64>,
    pub fullscreen: Option<bool>,
    pub idle: Option<bool>,
}

pub struct PlayerStateDiffer {
    inner: PlayerState,
}

fn diff<T: Clone>(this: &mut T, other: &T, eq: impl FnOnce(&T, &T) -> bool) -> Option<T> {
    if !eq(this, other) {
        other.clone_into(this);
        Some(other.clone())
    } else {
        None
    }
}

impl PlayerStateDiffer {
    pub fn new(state: PlayerState) -> Self {
        Self { inner: state }
    }
    pub fn get_diff(&mut self, new: &PlayerState) -> PlayerStateChanged {
        let playlist = diff(&mut self.inner.playlist, &new.playlist, Arc::ptr_eq);
        let current = diff(&mut self.inner.current, &new.current, Option::eq);
        let pause = diff(&mut self.inner.pause, &new.pause, bool::eq);
        let position = diff(&mut self.inner.position, &new.position, f64::eq);
        let fullscreen = diff(&mut self.inner.fullscreen, &new.fullscreen, bool::eq);
        let idle = diff(&mut self.inner.idle, &new.idle, bool::eq);
        PlayerStateChanged {
            current,
            pause,
            position,
            fullscreen,
            playlist,
            idle,
        }
    }
}
