use std::{sync::Arc, task::Poll};

use futures_util::Stream;
use jellyfin::items::MediaItem;
use libmpv::node::ToNode;
use tokio::{sync::mpsc, time::Interval};
use tracing::{Instrument, debug, error_span, instrument::Instrumented, warn};

use crate::{
    Command, PlayerState,
    mpv_stream::{MpvEvent, MpvStream, ObservedProperty},
};
use color_eyre::{Result, eyre::Context};

pin_project_lite::pin_project! {
    pub(crate) struct PollState{
       pub(crate) closed: bool,
       pub(crate) started: bool,
       pub(crate) initial_index:usize,
        #[pin]
       pub(crate) mpv: MpvStream,
       pub(crate) commands: mpsc::UnboundedReceiver<Command>,
       pub(crate) send: tokio::sync::watch::Sender<PlayerState>,
       pub(crate) send_timer: Interval,
       pub(crate) paused: bool,
       pub(crate) position: f64,
       pub(crate) index: usize,
       pub(crate) items: Vec<Arc<MediaItem>>,
    }
}

impl PollState {
    pub(crate) fn instrument(self) -> Instrumented<Self> {
        Instrument::instrument(self, error_span!("mpv-player"))
    }
}

trait ResExt {
    fn trace_error(self) -> ();
}

impl ResExt for Result<()> {
    fn trace_error(self) {
        if let Err(e) = self {
            warn!("Error handling mpv player: {e:?}")
        }
    }
}

impl Future for PollState {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        let span = error_span!("commands").entered();
        if !*this.closed {
            while let Poll::Ready(val) = this.commands.poll_recv(cx) {
                match val {
                    None | Some(Command::Close) => {
                        this.mpv.quit().context("quitting mpv").trace_error();
                        *this.closed = true;
                        break;
                    }
                    Some(Command::Play) => this.mpv.unpause().context("unpusing mpv").trace_error(),
                    Some(Command::Pause) => this.mpv.pause().context("pausing mpv").trace_error(),
                    Some(Command::PlayPause) => this
                        .mpv
                        .set_pause(!*this.paused)
                        .context("toggling pause on mpv")
                        .trace_error(),
                }
            }
        }
        span.exit();
        let span = error_span!("mpv-events").entered();
        let mut send = false;
        while let Poll::Ready(val) = this.mpv.as_mut().poll_next(cx) {
            match val {
                None => {
                    return Poll::Ready(());
                }
                Some(Err(e)) => warn!("Error form mpv: {e:?}"),
                Some(Ok(MpvEvent::StartFile(id))) => {
                    let id = usize::try_from(id.checked_sub(1).expect("This should be 1 indexed")).expect("This should never overflow, not even on 32-bit platforms (it is doubtful if this code will ever even be executed on a 32 bit system)");
                    if !*this.started {
                        *this.started = true;
                        if id != *this.initial_index {
                            debug!("initial index: {id}, should be {}", this.initial_index);
                            if let Err(e) = this.mpv.command(&[
                                c"playlist-play-index".to_node(),
                                i64::try_from(
                                    this.initial_index
                                        .checked_add(1)
                                        .expect("adding 1 should not overflow"),
                                )
                                .expect("initial index should be valid")
                                .to_node(),
                            ]) {
                                warn!("error forcing correct initial playlist entry: {e:?}")
                            } else {
                                debug!("set index to {}", this.initial_index);
                            }
                            continue;
                        }
                    }
                    *this.index = id;
                    send = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(true)))) => {
                    if *this.started {
                        this.mpv.quit().context("quitting mpv").trace_error();
                        *this.closed = true;
                    }
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(false)))) => {}
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(p)))) => {
                    *this.position = p
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(paused)))) => {
                    *this.paused = paused;
                    send = true;
                }
            }
        }
        span.exit();
        let span = error_span!("push-events").entered();
        if this.send_timer.poll_tick(cx).is_ready() || send {
            this.send
                .send(PlayerState {
                    index: *this.index,
                    current: this
                        .items
                        .get(*this.index)
                        .expect(
                            "this is only out of bounds if the playlist got out of sync with mpv",
                        )
                        .to_owned(),
                    pause: *this.paused,
                    position: *this.position,
                })
                .context("sending player state update")
                .trace_error();
        }
        span.exit();
        Poll::Pending
    }
}
