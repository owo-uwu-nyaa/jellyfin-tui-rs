use std::{ops::Deref, pin::pin, task::Poll};

use crate::Wrapper;
use color_eyre::Result;
pub use futures_channel::mpsc::SendError;
use futures_channel::mpsc::Sender;
pub use futures_util::{Sink, SinkExt, Stream, StreamExt};
use pin_project_lite::pin_project;
use spawn::Spawner;
use std::result::Result as StdResult;
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tracing::Span;

pin_project! {
    pub struct TaskSubmitter<A, W: Wrapper<A>> {
        wrapper: W,
        #[pin]
        sender: Sender<Result<W::F>>,
        spawner: Spawner,
        cancel: CancellationToken
    }
}

impl<A, W: Wrapper<A>> Clone for TaskSubmitter<A, W> {
    fn clone(&self) -> Self {
        Self {
            wrapper: self.wrapper,
            sender: self.sender.clone(),
            spawner: self.spawner.clone(),
            cancel: self.cancel.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Wrapped<W1, W2> {
    w1: W1,
    w2: W2,
}

impl<W1, W2, C> Wrapper<C> for Wrapped<W1, W2>
where
    W1: Wrapper<C>,
    W2: Wrapper<W1::F>,
{
    type F = W2::F;

    fn wrap(&self, val: C) -> Self::F {
        self.w2.wrap(self.w1.wrap(val))
    }
}

pin_project! {
    struct Cancelled<F>{
        #[pin]
        f:F,
        #[pin]
        cancel: WaitForCancellationFutureOwned,
    }
}

impl<F: Future<Output = ()>> Future for Cancelled<F> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let s = self.project();
        if let Poll::Ready(()) = s.cancel.poll(cx) {
            Poll::Ready(())
        } else {
            s.f.poll(cx)
        }
    }
}

impl<A, W: Wrapper<A>> TaskSubmitter<A, W> {
    pub fn wrap_with<AN, WN: Wrapper<AN, F = A>>(
        self,
        wrapper: WN,
    ) -> TaskSubmitter<AN, Wrapped<WN, W>> {
        TaskSubmitter {
            wrapper: Wrapped {
                w1: wrapper,
                w2: self.wrapper,
            },
            sender: self.sender,
            spawner: self.spawner,
            cancel: self.cancel,
        }
    }

    pub fn wrapper(&self) -> W {
        self.wrapper
    }

    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel
    }

    pub fn spawn_task(self, fut: impl Future<Output = Result<A>> + Send + 'static, span: Span) {
        let wrapper = self.wrapper;
        let mut sender = self.sender.clone();
        let cancel = self.cancel.clone().cancelled_owned();
        self.spawner.spawn(
            async move {
                let inner = async {
                    let _ = sender.feed(fut.await.map(|v| wrapper.wrap(v))).await;
                };
                Cancelled { f: inner, cancel }.await
            },
            span,
        );
    }
}

impl<A: Send, W: Wrapper<A>> TaskSubmitter<A, W> {
    pub fn spawn_stream(self, stream: impl Stream<Item = Result<A>> + Send + 'static, span: Span) {
        let wrapper = self.wrapper;
        let mut sender = self.sender.clone();
        let cancel = self.cancel.clone().cancelled_owned();
        self.spawner.spawn(
            async move {
                let inner = async {
                    let mut stream = pin!(stream);
                    while let Some(v) = stream.next().await {
                        if sender.feed(v.map(|v| wrapper.wrap(v))).await.is_err() {
                            break;
                        }
                    }
                };
                Cancelled { f: inner, cancel }.await
            },
            span,
        );
    }
}

impl<A, W: Wrapper<A>> Deref for TaskSubmitter<A, W> {
    type Target = Spawner;

    fn deref(&self) -> &Self::Target {
        &self.spawner
    }
}

impl<A, W: Wrapper<A>> Sink<Result<A>> for TaskSubmitter<A, W> {
    type Error = SendError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        self.project().sender.poll_ready(cx)
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Result<A>) -> StdResult<(), Self::Error> {
        let s = self.project();
        s.sender.start_send(item.map(|v| s.wrapper.wrap(v)))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        self.project().sender.poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        self.project().sender.poll_close(cx)
    }
}
