use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{self, Poll, Waker},
};

use parking_lot::Mutex;
use tracing::{instrument, trace};

pub(super) struct ImagesAvailableInner {
    available: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl ImagesAvailableInner {
    #[instrument(level = "trace", skip_all)]
    pub(super) fn wake(&self) {
        trace!("images available");
        if !self.available.load(Ordering::SeqCst)
            && !self.available.swap(true, Ordering::SeqCst)
            && let Some(waker) = self.waker.lock().take()
        {
            trace!("waking");
            waker.wake();
        }
    }
}

#[derive(Clone)]
pub struct ImagesAvailable {
    pub(super) inner: Arc<ImagesAvailableInner>,
}

impl Debug for ImagesAvailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagesAvailable")
            .field("available", &self.inner.available.load(Ordering::SeqCst))
            .finish()
    }
}

impl ImagesAvailable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ImagesAvailableInner {
                available: false.into(),
                waker: Mutex::new(None),
            }),
        }
    }
    pub fn wait_available(&self) -> ImagesAvailableFuture<'_> {
        ImagesAvailableFuture { inner: &self.inner }
    }
}

impl Default for ImagesAvailable {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ImagesAvailableFuture<'a> {
    inner: &'a ImagesAvailableInner,
}

impl Future for ImagesAvailableFuture<'_> {
    type Output = ();

    #[instrument(level = "trace", skip_all)]
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        if self.inner.available.swap(false, Ordering::SeqCst) {
            trace!("awakened");
            Poll::Ready(())
        } else {
            let mut waker = self.inner.waker.lock();
            if self.inner.available.swap(false, Ordering::SeqCst) {
                trace!("awakened after lock");
                Poll::Ready(())
            } else {
                *waker = Some(cx.waker().clone());
                trace!("sleeping");
                Poll::Pending
            }
        }
    }
}
