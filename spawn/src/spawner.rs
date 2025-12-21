use tokio::{sync::mpsc::UnboundedSender, task::JoinSet};
use tracing::{Instrument, Span, warn};

pub type JoinSetCallback = Box<dyn FnOnce(&mut JoinSet<()>) + Send + 'static>;

#[derive(Clone)]
pub struct Spawner {
    pub(crate) sender: UnboundedSender<JoinSetCallback>,
}

impl Spawner {
    pub fn raw(&self, f: impl FnOnce(&mut JoinSet<()>) + Send + 'static) {
        let _ = self.sender.send(Box::new(f));
    }

    pub fn spawn_bare(&self, fut: impl Future<Output = ()> + Send + 'static) {
        self.raw(move |join_set| {
            join_set.spawn(fut);
        });
    }
    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static, span: Span) {
        self.spawn_bare(fut.instrument(span));
    }
    pub fn spawn_res<T>(
        &self,
        fut: impl Future<Output = color_eyre::Result<T>> + Send + 'static,
        span: Span,
    ) {
        self.spawn(
            async move {
                if let Err(e) = fut.await {
                    warn!("error returned from task: {e:?}")
                }
            },
            span,
        );
    }
}
