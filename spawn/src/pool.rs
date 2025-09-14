use std::task::Poll;

use pin_project_lite::pin_project;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinSet};
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tracing::{Instrument, Span};

pin_project! {
    pub struct Pool {
        recv: UnboundedReceiver<Box<dyn FnOnce(&mut JoinSet<()>)>>,
        pool: JoinSet<()>,
        closed: bool,
        cancellation: CancellationToken,
        #[pin]
        cancellation_fut : WaitForCancellationFutureOwned,
    }
}

impl Future for Pool {
    type Output = bool;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.as_mut().project();
        if *this.closed {
            loop {
                match this.pool.poll_join_next(cx) {
                    Poll::Pending => break Poll::Pending,
                    Poll::Ready(None) => {
                        crate::spawner::remove_current_sender();
                        break Poll::Ready(true);
                    }
                    Poll::Ready(Some(_)) => {}
                }
            }
        } else if this.cancellation_fut.poll(cx).is_ready() {
            this.pool.abort_all();
            *this.closed = true;
            self.poll(cx)
        } else {
            let empty = loop {
                let pool_res = this.pool.poll_join_next(cx);
                match pool_res {
                    Poll::Pending => break false,
                    Poll::Ready(None) => break true,
                    Poll::Ready(Some(Ok(()))) => {}
                    Poll::Ready(Some(Err(e))) => {
                        if e.is_panic() {
                            this.pool.abort_all();
                            *this.closed = true;
                            this.cancellation.cancel();
                            return self.poll(cx);
                        }
                    }
                }
            };
            while let Poll::Ready(Some(job)) = this.recv.poll_recv(cx) {
                job(this.pool)
            }
            if empty && this.pool.is_empty() {
                crate::spawner::remove_current_sender();
                Poll::Ready(false)
            } else {
                Poll::Pending
            }
        }
    }
}

pub fn run_with_spawner(
    f: impl Future<Output = ()> + Send + 'static,
    cancel: CancellationToken,
    span: Span,
) -> Pool {
    let (send, recv) = tokio::sync::mpsc::unbounded_channel();
    crate::spawner::set_sender(send);
    let mut pool = JoinSet::new();
    pool.spawn(f.instrument(span));
    let fut = cancel.clone().cancelled_owned();
    Pool {
        recv,
        pool,
        closed: false,
        cancellation: cancel,
        cancellation_fut: fut,
    }
}
