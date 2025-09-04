use std::{ffi::CString, pin::Pin, sync::Arc, task::Poll, time::Duration};

use futures_util::{Stream, StreamExt, stream::FusedStream};
use jellyfin::{
    Auth, JellyfinClient,
    items::{ItemType, MediaItem},
    playback_status::ProgressBody,
    socket::JellyfinWebSocket,
};
use libmpv::{
    Mpv,
    events::EventContextAsync,
    node::{BorrowingCPtr, BorrowingMpvNodeMap, ToNode},
};
use tokio::{
    task::JoinSet,
    time::{Interval, MissedTickBehavior},
};
use tracing::{debug, error, info, instrument, trace};

use crate::Config;

use super::mpv_stream::{MpvEvent, MpvStream, ObservedProperty};

use color_eyre::eyre::{Context, OptionExt, Report, Result, eyre};

pub struct Player<'j> {
    mpv: Option<MpvStream>,
    items: Vec<MediaItem>,
    index: Option<usize>,
    id: Option<Arc<String>>,
    position: Option<f64>,
    paused: bool,
    send_timer: Interval,
    join: JoinSet<()>,
    jellyfin: &'j JellyfinClient<Auth>,
    jellyfin_socket: Pin<&'j mut JellyfinWebSocket>,
    was_running: bool,
    last_position: Option<f64>,
    last_index: Option<usize>,
    done: bool,
    finished: bool,
    playback_started: bool,
    initial_index: i64,
}

impl FusedStream for Player<'_> {
    fn is_terminated(&self) -> bool {
        self.finished
    }
}

#[inline]
fn poll_state(state: &mut Player<'_>, cx: &mut std::task::Context<'_>) -> Result<Poll<Option<()>>> {
    loop {
        match state.jellyfin_socket.as_mut().poll_next_unpin(cx) {
            Poll::Pending => break,
            Poll::Ready(None) => {
                break;
            }
            Poll::Ready(Some(v)) => {
                info!("websocket message: {v:#?}");
            }
        }
    }
    let res = 'res: {
        while let Poll::Ready(res) = state.join.poll_join_next(cx) {
            match res {
                Some(Ok(())) => {}
                Some(Err(e)) => {
                    state.finished = true;
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
                        if !state.playback_started {
                            state.playback_started = true;
                            debug!("initial index: {index}, should be {}", state.initial_index);
                            if index != state.initial_index {
                                mpv.command(&[
                                    c"playlist-play-index".to_node(),
                                    (state.initial_index).to_node(),
                                ])?;
                                debug!("setting index to {}", state.initial_index);
                                continue;
                            }
                        }
                        info!("new index: {index}");
                        send_playing_stopped(
                            state.id.as_ref(),
                            state.position,
                            state.paused,
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
                            if let Err(e) = async { started?.send(&id).await }.await {
                                error!("error sending playback started: {e:?}")
                            }
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
                                    state.paused,
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
                    Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(pause)))) => {
                        state.paused = pause;
                    }

                    Some(Err(e)) => return Err(e),
                    None => {
                        info!("player quit");
                        send_playing_stopped(
                            state.id.as_ref(),
                            state.position,
                            state.paused,
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
        if state.send_timer.poll_tick(cx).is_ready()
            && let (Some(id), Some(pos)) = (state.id.as_ref(), state.position)
            && (state.index != state.last_index || state.position != state.last_position)
        {
            debug!("updating playback progress");
            state.last_index = state.index;
            state.last_position = state.position;
            let progress = state.jellyfin.prepare_set_playing_progress();
            let id = id.clone();
            let paused = state.paused;
            state.join.spawn(async move {
                if let Err(e) = async {
                    progress?
                        .send(&ProgressBody {
                            item_id: &id,
                            is_paused: paused,
                            position_ticks: (pos * 10000000.0) as u64,
                        })
                        .await
                }
                .await
                {
                    error!("error updating playback progress: {e:?}")
                }
            });
        }
        Poll::Pending
    };
    Ok(res)
}

fn send_playing_stopped(
    id: Option<&Arc<String>>,
    position: Option<f64>,
    paused: bool,
    jellyfin: &JellyfinClient<Auth>,
    join: &mut JoinSet<()>,
) {
    if let (Some(id), Some(pos)) = (id, position) {
        let finished = jellyfin.prepare_set_playing_stopped();
        let id = id.clone();
        join.spawn(async move {
            if let Err(e) = async {
                finished?
                    .send(&ProgressBody {
                        item_id: &id,
                        position_ticks: (pos * 10000000.0) as u64,
                        is_paused: paused,
                    })
                    .await
            }
            .await
            {
                error!("error sending stop message: {e:?}")
            }
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
    pub async fn new(
        jellyfin: &'j JellyfinClient,
        jellyfin_socket: Pin<&'j mut JellyfinWebSocket>,
        config: &Config,
        items: Vec<MediaItem>,
        index: usize,
    ) -> Result<Self> {
        let mpv = MpvStream::new(jellyfin, config)?;

        let position = items[index]
            .user_data
            .as_ref()
            .ok_or_eyre("user data missing")?
            .playback_position_ticks
            / 10000000;

        for item in items[0..index].iter() {
            append(&mpv, jellyfin, item)?
        }
        debug!("previous files added");
        let uri = jellyfin.get_video_uri(&items[index])?.to_string();
        debug!("adding {uri} to queue and play it");
        mpv.command(&[
            c"loadfile".to_node(),
            CString::new(uri)
                .context("converting video url to cstr")?
                .to_node(),
            c"append-play".to_node(),
            0i64.to_node(),
            BorrowingMpvNodeMap::new(
                &[
                    BorrowingCPtr::new(c"start"),
                    BorrowingCPtr::new(c"force-media-title"),
                ],
                &[
                    CString::new(position.to_string())
                        .context("converting start to cstr")?
                        .to_node(),
                    name(&items[index])?.to_node(),
                ],
            )
            .to_node(),
        ])
        .context("added main item")?;
        debug!("main file added to playlist at index {index}");
        for item in items[index + 1..].iter() {
            append(&mpv, jellyfin, item)?
        }
        debug!("later files added");
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
            jellyfin_socket,
            was_running: false,
            last_position: None,
            last_index: None,
            done: false,
            finished: false,
            id: None,
            playback_started: false,
            initial_index: index as i64,
            paused: true,
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

fn append(mpv: &Mpv<EventContextAsync>, jellyfin: &JellyfinClient, item: &MediaItem) -> Result<()> {
    let uri = jellyfin.get_video_uri(item)?.to_string();
    debug!("adding {uri} to queue");
    mpv.command(&[
        c"loadfile".to_node(),
        CString::new(uri)
            .context("converting video url to cstr")?
            .to_node(),
        c"append".to_node(),
        0i64.to_node(),
        BorrowingMpvNodeMap::new(
            &[BorrowingCPtr::new(c"force-media-title")],
            &[name(item)?.to_node()],
        )
        .to_node(),
    ])?;

    Ok(())
}

fn name(item: &MediaItem) -> Result<CString> {
    let name = match &item.item_type {
        ItemType::Movie => item.name.clone(),
        ItemType::Episode {
            season_id: _,
            season_name: _,
            series_id: _,
            series_name,
        } => {
            if let Some(i) = item.episode_index {
                let index = i.to_string();
                //dumb check if name is usefull
                let (mut string, episode) = if item.name.contains(&index) {
                    (series_name.clone(), false)
                } else {
                    (item.name.clone(), true)
                };
                string.push(' ');
                if episode {
                    string.push('(');
                }
                if let Some(i) = item.season_index {
                    string.push('S');
                    string += &i.to_string();
                }
                string.push('E');
                string += &index;
                if episode {
                    string.push(')');
                }
                string
            } else {
                item.name.clone()
            }
        }
        t => return Err(eyre!("unsupported item type: {t:?}")),
    };
    Ok(CString::new(name)?)
}
