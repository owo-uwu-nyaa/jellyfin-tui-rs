use std::{ffi::CString, sync::Arc, task::Poll, time::Duration};

use futures_util::{Stream, StreamExt, stream::FusedStream};
use jellyfin::{Auth, JellyfinClient, items::MediaItem, playback_status::ProgressBody};
use libmpv::node::{BorrowingCPtr, BorrowingMpvNodeMap, ToNode};
use tokio::{
    task::JoinSet,
    time::{Interval, MissedTickBehavior},
};
use tracing::{debug, info, instrument, trace};

use crate::TuiContext;

use super::mpv_stream::{MpvEvent, MpvStream, ObservedProperty};

use color_eyre::eyre::{Context, OptionExt, Report, Result};

pub struct Player<'j> {
    mpv: Option<MpvStream>,
    items: Vec<MediaItem>,
    index: Option<usize>,
    id: Option<Arc<String>>,
    position: Option<f64>,
    send_timer: Interval,
    join: JoinSet<Result<()>>,
    jellyfin: &'j JellyfinClient<Auth>,
    was_running: bool,
    last_position: Option<f64>,
    last_index: Option<usize>,
    done: bool,
    finished: bool,
}

impl FusedStream for Player<'_> {
    fn is_terminated(&self) -> bool {
        self.finished
    }
}

#[inline]
fn poll_state(state: &mut Player<'_>, cx: &mut std::task::Context<'_>) -> Result<Poll<Option<()>>> {
    let res = 'res: {
        while let Poll::Ready(res) = state.join.poll_join_next(cx) {
            match res {
                Some(Ok(Ok(()))) => {}
                Some(Ok(Err(e))) => {
                    state.finished = true;
                    return Err(e);
                }
                Some(Err(e)) => {
                    return Err(Report::new(e));
                }
                None => {
                    if state.done {
                        state.finished = true;
                        break 'res Poll::Ready(None);
                    } else {
                        break;
                    }
                }
            }
        }
        if let Some(mpv) = state.mpv.as_mut() {
            while let Poll::Ready(res) = mpv.poll_next_unpin(cx) {
                match res {
                    Some(Ok(MpvEvent::StartFile(index))) => {
                        //mpv index is 1 based
                        let index = index - 1;
                        info!("new index: {index}");
                        send_playing_stopped(
                            state.id.as_ref(),
                            state.position,
                            state.jellyfin,
                            &mut state.join,
                        );
                        let index = index.try_into().context("converting index to unsigned")?;
                        state.index = Some(index);
                        state.position = None;
                        let id = Arc::new(
                            state
                                .items
                                .get(index)
                                .ok_or_eyre("item index out of bounds")?
                                .id
                                .clone(),
                        );
                        state.id = Some(id.clone());
                        let started = state.jellyfin.prepare_set_playing();
                        state.join.spawn(async move {
                            started.send(&id).await?;
                            Ok(())
                        });
                        break 'res Poll::Ready(Some(()));
                    }
                    Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(pos)))) => {
                        trace!("position updated to {pos}");
                        state.position = Some(pos);
                    }
                    Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(idle)))) => {
                        if idle {
                            if state.was_running {
                                info!("player idle");
                                send_playing_stopped(
                                    state.id.as_ref(),
                                    state.position,
                                    state.jellyfin,
                                    &mut state.join,
                                );
                                state.mpv = None;
                                state.id = None;
                                state.index = None;
                                state.position = None;
                                state.done = true;
                                break 'res Poll::Ready(Some(()));
                            }
                        } else {
                            state.was_running = true;
                        }
                    }
                    Some(Err(e)) => return Err(e),
                    None => {
                        info!("player quit");
                        send_playing_stopped(
                            state.id.as_ref(),
                            state.position,
                            state.jellyfin,
                            &mut state.join,
                        );
                        state.mpv = None;
                        state.id = None;
                        state.index = None;
                        state.position = None;
                        state.done = true;
                        break 'res Poll::Ready(Some(()));
                    }
                }
            }
        }
        if state.send_timer.poll_tick(cx).is_ready() {
            if let (Some(id), Some(pos)) = (state.id.as_ref(), state.position) {
                if state.index != state.last_index || state.position != state.last_position {
                    debug!("updating playback progress");
                    state.last_index = state.index;
                    state.last_position = state.position;
                    let progress = state.jellyfin.prepare_set_playing_progress();
                    let id = id.clone();
                    state.join.spawn(async move {
                        progress
                            .send(&ProgressBody {
                                item_id: &id,
                                position_ticks: (pos * 10000000.0) as u64,
                            })
                            .await?;
                        Ok(())
                    });
                }
            }
        }
        Poll::Pending
    };
    Ok(res)
}

fn send_playing_stopped(
    id: Option<&Arc<String>>,
    position: Option<f64>,
    jellyfin: &JellyfinClient<Auth>,
    join: &mut JoinSet<Result<()>>,
) {
    if let (Some(id), Some(pos)) = (id, position) {
        let finished = jellyfin.prepare_set_playing_stopped();
        let id = id.clone();
        join.spawn(async move {
            finished
                .send(&ProgressBody {
                    item_id: &id,
                    position_ticks: (pos * 10000000.0) as u64,
                })
                .await?;
            Ok(())
        });
    }
}

impl Stream for Player<'_> {
    type Item = Result<()>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.finished {
            Poll::Ready(None)
        } else {
            match poll_state(&mut self, cx) {
                Err(e) => {
                    self.finished = true;
                    Poll::Ready(Some(Err(e)))
                }
                Ok(Poll::Ready(Some(()))) => Poll::Ready(Some(Ok(()))),
                Ok(Poll::Ready(None)) => Poll::Ready(None),
                Ok(Poll::Pending) => Poll::Pending,
            }
        }
    }
}

pub enum PlayerState<'p> {
    Initializing,
    Playing(&'p MediaItem),
    Exiting,
}

impl<'j> Player<'j> {
    #[instrument(skip_all)]
    pub fn new(
        cx: &TuiContext,
        jellyfin: &'j JellyfinClient<Auth>,
        items: Vec<MediaItem>,
        index: usize,
    ) -> Result<Self> {
        let mpv = MpvStream::new(cx)?;

        let position = items[index]
            .user_data
            .as_ref()
            .ok_or_eyre("user data missing")?
            .playback_position_ticks
            / 10000000;
        for item in items[0..index].iter() {
            mpv.playlist_append(
                &CString::new(jellyfin.get_video_url(item))
                    .context("converting video url to cstr")?,
            )?;
        }
        debug!("previous files added");

        mpv.command(&[
            c"loadfile".to_node(),
            CString::new(jellyfin.get_video_url(&items[index]))
                .context("converting video url to cstr")?
                .to_node(),
            //c"https://www.youtube.com/watch?v=Q9kLrlqoMfk".to_node(),
            c"replace".to_node(),
            0i64.to_node(),
            BorrowingMpvNodeMap::new(
                &[BorrowingCPtr::new(c"start")],
                &[CString::new(format!("{position}"))
                    .context("converting start to cstr")?
                    .to_node()],
            )
            .to_node(),
        ])
        .context("adding to playlist")?;
        debug!("main file added to playlist");
        for item in items[index + 1..].iter() {
            mpv.playlist_append(
                &CString::new(jellyfin.get_video_url(item))
                    .context("converting video url to cstr")?,
            )?;
        }
        debug!("later files added");

        debug!("mpv finished");
        let mut send_timer = tokio::time::interval(Duration::from_secs(10));
        send_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        Ok(Self {
            mpv: Some(mpv),
            items,
            index: None,
            position: None,
            send_timer,
            join: JoinSet::new(),
            jellyfin,
            was_running: false,
            last_position: None,
            last_index: None,
            done: false,
            finished: false,
            id: None,
        })
    }

    pub fn state(&self) -> Result<PlayerState<'_>> {
        Ok(if self.done {
            PlayerState::Exiting
        } else if let Some(index) = self.index {
            PlayerState::Playing(self.items.get(index).ok_or_eyre("index out of bounds")?)
        } else {
            PlayerState::Initializing
        })
    }
}
