#![allow(clippy::too_many_arguments)]

use std::{ffi::CString, sync::Arc, task::Poll};

use color_eyre::eyre::{bail, eyre};
use futures_util::Stream;
use jellyfin::items::MediaItem;
use jellyfin::{JellyfinClient, items::ItemType};
use libmpv::node::{BorrowingCPtr, BorrowingMpvNodeMap, ToNode};
use tokio::{sync::mpsc, time::Interval};
use tokio_util::sync::WaitForCancellationFutureOwned;
use tracing::info;
use tracing::{Instrument, debug, error_span, instrument, instrument::Instrumented, warn};

use crate::create::set_playlist;
use crate::mpv_stream::ClientCommand;
use crate::{
    Command, PlayerState, PlaylistItem,
    mpv_stream::{MpvEvent, MpvStream, ObservedProperty},
};
use crate::{PlaylistItemId, PlaylistItemIdGen};
use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};

pin_project_lite::pin_project! {
    pub(crate) struct PollState{
        pub(crate) closed: bool,
        #[pin]
        pub(crate) mpv: MpvStream,
        pub(crate) jellyfin: JellyfinClient,
        #[pin]
        pub(crate) stop: WaitForCancellationFutureOwned,
        pub(crate) commands: mpsc::UnboundedReceiver<Command>,
        pub(crate) send: tokio::sync::watch::Sender<PlayerState>,
        pub(crate) send_timer: Interval,
        pub(crate) paused: bool,
        pub(crate) position: f64,
        pub(crate) index: Option<usize>,
        pub(crate) fullscreen: bool,
        pub(crate) minimized: bool,
        pub(crate) idle: bool,
        pub(crate) playlist: Arc<Vec<Arc<PlaylistItem>>>,
        pub(crate) playlist_id_gen: PlaylistItemIdGen,
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
        let mut send = false;
        let span = error_span!("commands").entered();
        if !*this.closed {
            if this.stop.poll(cx).is_ready() {
                info!("mpv stopped");
                this.mpv.quit().context("quitting mpv").trace_error();
                *this.closed = true;
            } else {
                while let Poll::Ready(val) = this.commands.poll_recv(cx) {
                    match val {
                        None => {
                            info!("all senders are closed");
                            this.mpv.quit().context("quitting mpv").trace_error();
                            *this.closed = true;
                            break;
                        }
                        Some(Command::Pause(pause)) => this
                            .mpv
                            .set_pause(pause)
                            .context("setting pause on mpv")
                            .trace_error(),
                        Some(Command::Fullscreen(fullscreen)) => this
                            .mpv
                            .set_fullscreen(fullscreen)
                            .context("setting fullscreen")
                            .trace_error(),
                        Some(Command::Minimized(minimized)) => this
                            .mpv
                            .set_minimized(minimized)
                            .context("setting window minimized")
                            .trace_error(),
                        Some(Command::Next) => this
                            .mpv
                            .playlist_next_force()
                            .context("skipping to next item")
                            .trace_error(),
                        Some(Command::Previous) => this
                            .mpv
                            .playlist_previous_weak()
                            .context("moving to previous item")
                            .trace_error(),
                        Some(Command::Seek(seek)) => this
                            .mpv
                            .seek_absolute(seek)
                            .context("seeking")
                            .trace_error(),
                        Some(Command::Play(id)) => {
                            if let Some(index) = index_of(this.playlist, id) {
                                match i64::try_from(index).context("Index is an invalid index") {
                                    Err(e) => warn!("error converting {index}\n{e:?}"),
                                    Ok(index) => this
                                        .mpv
                                        .playlist_play_index(index)
                                        .context("setting current playlist index")
                                        .trace_error(),
                                }
                            }
                        }
                        Some(Command::AddTrack { item, after, play }) => {
                            insert_at(
                                this.playlist,
                                &this.mpv,
                                this.jellyfin,
                                item,
                                after,
                                this.playlist_id_gen,
                                play,
                                &mut send,
                            )
                            .context("adding item to playlist")
                            .trace_error();
                        }
                        Some(Command::Stop) => {
                            stop(&this.mpv, this.playlist, this.index, &mut send)
                                .context("stopping player")
                                .trace_error();
                        }
                        Some(Command::ReplacePlaylist { items, first }) => {
                            replace_playlist(
                                &this.mpv,
                                this.jellyfin,
                                this.playlist_id_gen,
                                this.playlist,
                                items,
                                first,
                                &mut send,
                                this.index,
                            )
                            .trace_error();
                        }
                        Some(Command::Remove(id)) => {
                            remove_playlist_item(
                                this.playlist,
                                &this.mpv,
                                id,
                                &mut send,
                                this.index,
                            )
                            .trace_error();
                        }
                    }
                }
            }
        }
        span.exit();
        let span = error_span!("mpv-events").entered();
        while let Poll::Ready(val) = this.mpv.as_mut().poll_next(cx) {
            match val {
                None => {
                    info!("mpv events exhausted");
                    return Poll::Ready(());
                }
                Some(Err(e)) => warn!("Error form mpv: {e:?}"),
                Some(Ok(MpvEvent::StartFile(id))) => {
                    let id = usize::try_from(id.checked_sub(1).expect("This should be 1 indexed")).expect("This should never overflow, not even on 32-bit platforms (it is doubtful if this code will ever even be executed on a 32 bit system)");
                    *this.index = Some(id);
                    send = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(idle)))) => {
                    *this.idle = idle;
                    if idle {
                        *this.index = None;
                    }
                    send = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(p)))) => {
                    *this.position = p
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(paused)))) => {
                    *this.paused = paused;
                    send = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Fullscreen(fullscreen)))) => {
                    *this.fullscreen = fullscreen;
                    send = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Minimized(minimized)))) => {
                    *this.minimized = minimized;
                    send = true;
                }
                Some(Ok(MpvEvent::Command(ClientCommand::Stop))) => {
                    stop(&this.mpv, this.playlist, this.index, &mut send)
                        .context("stopping player")
                        .trace_error();
                }
            }
        }
        span.exit();
        let span = error_span!("push-events").entered();
        if this.send_timer.poll_tick(cx).is_ready() || send {
            this.send
                .send(PlayerState {
                    playlist: this.playlist.clone(),
                    current: *this.index,
                    pause: *this.paused,
                    position: *this.position,
                    fullscreen: *this.fullscreen,
                    minimized: *this.minimized,
                    idle: *this.idle,
                })
                .context("sending player state update")
                .trace_error();
        }
        span.exit();
        Poll::Pending
    }
}

fn stop(
    mpv: &MpvStream,
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    index: &mut Option<usize>,
    send: &mut bool,
) -> Result<()> {
    mpv.stop()?;
    *playlist = Arc::new(Vec::new());
    *index = None;
    *send = true;
    //mpv.set_minimized(true)?;
    Ok(())
}

fn remove_playlist_item(
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    mpv: &MpvStream,
    id: PlaylistItemId,
    send: &mut bool,
    cur_index: &mut Option<usize>,
) -> Result<()> {
    let index = index_of(playlist, id).ok_or_eyre("no such playlist item")?;
    mpv.playlist_remove_index(index.try_into().context("converting index to i64")?)
        .context("removing item from mpv playlist")?;
    Arc::make_mut(playlist).remove(index);
    *send = true;
    if *cur_index == Some(index) {
        *cur_index = None
    }
    Ok(())
}

fn replace_playlist(
    mpv: &MpvStream,
    jellyfin: &JellyfinClient,
    playlist_id_gen: &mut PlaylistItemIdGen,
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    items: Vec<MediaItem>,
    first: usize,
    send: &mut bool,
    index: &mut Option<usize>,
) -> Result<()> {
    if first >= items.len() {
        bail!("could not set playlist because first {first} is out of bounds.")
    }
    mpv.playlist_clear()?;
    let playlist = if let Some(playlist) = Arc::get_mut(playlist) {
        playlist.clear();
        playlist
    } else {
        *playlist = Arc::new(Vec::new());
        Arc::get_mut(playlist).expect("just created new playlist")
    };
    *index = None;
    *send = true;
    *playlist =
        set_playlist(mpv, jellyfin, playlist_id_gen, items, first).context("replacing playlist")?;
    mpv.playlist_play_index(first.try_into()?)?;
    Ok(())
}

fn insert_at(
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    mpv: &MpvStream,
    jellyfin: &JellyfinClient,
    item: Box<MediaItem>,
    after: Option<PlaylistItemId>,
    mk_id: &mut PlaylistItemIdGen,
    play: bool,
    send: &mut bool,
) -> Result<()> {
    let uri = jellyfin.get_video_uri(&item)?.to_string();

    let index = if let Some(id) = after {
        index_of(playlist, id).ok_or_eyre("could not find this item id!")?
    } else {
        0
    };

    let position = item
        .user_data
        .as_ref()
        .ok_or_eyre("user data missing")?
        .playback_position_ticks
        / 10000000;

    debug!("adding {uri} to queue");
    let at = i64::try_from(index).context("converting index to i64")?;
    mpv.command(&[
        c"loadfile".to_node(),
        CString::new(uri)
            .context("converting video url to cstr")?
            .to_node(),
        at.to_node(),
        BorrowingMpvNodeMap::new(
            &[
                BorrowingCPtr::new(c"start"),
                BorrowingCPtr::new(c"force-media-title"),
            ],
            &[
                CString::new(position.to_string())
                    .context("converting start to cstr")?
                    .to_node(),
                name(&item)?.to_node(),
            ],
        )
        .to_node(),
    ])?;
    let id = mk_id.next();
    Arc::make_mut(playlist).insert(index, Arc::new(PlaylistItem { item: *item, id }));
    *send = true;
    if play {
        mpv.playlist_play_index(at).context("playing new item")?
    }
    Ok(())
}

#[instrument(skip_all)]
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

fn index_of(playlist: &[Arc<PlaylistItem>], id: PlaylistItemId) -> Option<usize> {
    playlist
        .iter()
        .filter(|i| i.id == id)
        .enumerate()
        .next()
        .map(|(i, _)| i)
}
