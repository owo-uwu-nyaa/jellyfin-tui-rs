#![allow(clippy::too_many_arguments)]

use std::mem;
use std::{ffi::CString, sync::Arc, task::Poll};

use color_eyre::eyre::{bail, eyre};
use futures_util::Stream;
use jellyfin::items::MediaItem;
use jellyfin::{JellyfinClient, items::ItemType};
use libmpv::Mpv;
use libmpv::events::EventContextAsync;
use libmpv::node::{BorrowingCPtr, MpvNode, MpvNodeMapRef, ToNode};
use tokio::{
    sync::{broadcast, mpsc},
    time::Interval,
};
use tokio_util::sync::WaitForCancellationFutureOwned;
use tracing::{Instrument, debug, error_span, instrument, instrument::Instrumented, warn};
use tracing::{info, trace};

use crate::create::set_playlist;
use crate::mpv_stream::ClientCommand;
use crate::state::EventReceiver;
use crate::{
    Command, PlayerState, PlaylistItem,
    mpv_stream::{MpvEvent, MpvStream, ObservedProperty},
};
use crate::{Events, PlaylistItemId, PlaylistItemIdGen};
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
        pub(crate) position_send_timer: Interval,
        pub(crate) paused: bool,
        pub(crate) position: f64,
        pub(crate) speed: f64,
        pub(crate) volume: i64,
        pub(crate) index: Option<usize>,
        pub(crate) fullscreen: bool,
        pub(crate) minimized: bool,
        pub(crate) idle: bool,
        pub(crate) playlist: Arc<Vec<Arc<PlaylistItem>>>,
        pub(crate) playlist_id_gen: PlaylistItemIdGen,
        pub(crate) seeked: bool,
        pub(crate) send_events: broadcast::Sender<Events>,
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

fn extract_id(download_url: &str) -> &str {
    let id_part = download_url
        .rsplit("/Items/")
        .next()
        .expect("Items part not present in url");
    id_part
        .split('/')
        .next()
        .expect("no item id after last /Items")
}

fn assert_shadow_playlist_state(
    mpv: &Mpv<EventContextAsync>,
    shadow: &[Arc<PlaylistItem>],
) -> Result<()> {
    let prop: MpvNode = mpv.get_property("playlist")?;
    let mut mpv_playlist = prop
        .as_ref()
        .to_array()
        .expect("property should be an array")
        .into_iter()
        .flat_map(|v| v.to_map().expect("playlist item should be a map"))
        .filter_map(|(k, v)| if k == c"filename" { Some(v) } else { None })
        .map(|s| s.to_str().expect("filename should be a str"))
        .map(extract_id);
    let mut shadow_playlist = shadow.iter().map(|i| i.item.id.as_str());
    for index in 0usize.. {
        let mpv = mpv_playlist.next();
        let shadow = shadow_playlist.next();
        match (mpv, shadow) {
            (None, None) => break,
            (Some(_), None) => panic!(
                "The shadow playlist is shorter than the mpv internal playlist. index: {index}"
            ),
            (None, Some(_)) => {
                panic!("The mpv playlist state is shorter than the shadow playlist. index: {index}")
            }
            (Some(mpv), Some(shadow)) => assert_eq!(
                mpv, shadow,
                "mismatch between mpv and shadow playlist at index {index}"
            ),
        }
    }
    Ok(())
}

trait TraceSendError {
    fn trace_send_error(self);
}
impl<E, T> TraceSendError for std::result::Result<T, E> {
    fn trace_send_error(self) {
        if self.is_err() {
            trace!("unable to send message to non-existing peer.")
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
                        Some(Command::SeekRelative(seek)) => this
                            .mpv
                            .seek(seek, c"relative")
                            .context("seeking relative")
                            .trace_error(),
                        Some(Command::Play(id)) => {
                            if let Some(index) = index_of(this.playlist, id) {
                                match i64::try_from(index).context("Index is an invalid index") {
                                    Err(e) => warn!("error converting {index}\n{e:?}"),
                                    Ok(index) => play_index(&this.mpv, index).trace_error(),
                                }
                            }
                        }
                        Some(Command::Speed(speed)) => this
                            .mpv
                            .set_property(c"speed", speed)
                            .context("setting playback speed")
                            .trace_error(),
                        Some(Command::AddTrack { item, after, play }) => {
                            insert_at(
                                this.playlist,
                                &this.mpv,
                                this.jellyfin,
                                item,
                                after,
                                this.playlist_id_gen,
                                play,
                                this.send_events,
                            )
                            .context("adding item to playlist")
                            .trace_error();
                        }
                        Some(Command::Stop) => {
                            stop(&this.mpv, this.playlist, this.index, this.send_events)
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
                                this.send_events,
                                this.index,
                            )
                            .trace_error();
                        }
                        Some(Command::Remove(id)) => {
                            remove_playlist_item(
                                this.playlist,
                                &this.mpv,
                                id,
                                this.send_events,
                                this.index,
                            )
                            .trace_error();
                        }
                        Some(Command::TogglePause) => {
                            this.mpv
                                .set_pause(!*this.paused)
                                .context("toggle pause on player")
                                .trace_error();
                        }
                        Some(Command::Volume(volume)) => this
                            .mpv
                            .set_property(c"volume", volume)
                            .context("setting volume")
                            .trace_error(),
                        Some(Command::GetEventReceiver(sender)) => {
                            sender
                                .send(EventReceiver {
                                    state: PlayerState {
                                        playlist: this.playlist.clone(),
                                        current: *this.index,
                                        pause: *this.paused,
                                        stopped: *this.idle,
                                        position: *this.position,
                                        speed: *this.speed,
                                        fullscreen: *this.fullscreen,
                                        volume: *this.volume,
                                    },
                                    receive: this.send_events.subscribe(),
                                })
                                .trace_send_error();
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
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::PlaylistPos(position)))) => {
                    assert_shadow_playlist_state(&this.mpv, this.playlist).trace_error();
                    *this.index = if position == -1 {
                        None
                    } else {
                        match usize::try_from(position)
                            .context("converting playlist index to usize")
                        {
                            Ok(v) => Some(v),
                            Err(e) => {
                                Err(e).trace_error();
                                None
                            }
                        }
                    };
                    this.send_events
                        .send(Events::Current(*this.index))
                        .trace_send_error();
                    *this.position = 0.0;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(idle)))) => {
                    *this.idle = idle;
                    if idle {
                        *this.index = None;
                        this.send_events
                            .send(Events::Current(None))
                            .trace_send_error();
                    }
                    this.send_events
                        .send(Events::Stopped(idle))
                        .trace_send_error();
                }
                Some(Ok(MpvEvent::Seek)) => {
                    *this.seeked = true;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(pos)))) => {
                    let old = mem::replace(this.position, pos);
                    //seek if seek event or jump greater than 5 seconds
                    if mem::replace(this.seeked, false) || (old - pos).abs() > 5.0 {
                        this.send_events.send(Events::Seek(pos)).trace_send_error();
                    }
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(paused)))) => {
                    *this.paused = paused;
                    this.send_events
                        .send(Events::Paused(paused))
                        .trace_send_error();
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Fullscreen(fullscreen)))) => {
                    *this.fullscreen = fullscreen;
                    this.send_events
                        .send(Events::Fullscreen(fullscreen))
                        .trace_send_error();
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Minimized(minimized)))) => {
                    *this.minimized = minimized;
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Speed(speed)))) => {
                    *this.speed = speed;
                    this.send_events
                        .send(Events::Speed(speed))
                        .trace_send_error();
                }
                Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Volume(volume)))) => {
                    *this.volume = volume;
                    this.send_events
                        .send(Events::Volume(volume))
                        .trace_send_error();
                }
                Some(Ok(MpvEvent::Command(ClientCommand::Stop))) => {
                    stop(&this.mpv, this.playlist, this.index, this.send_events)
                        .context("stopping player")
                        .trace_error();
                }
            }
        }
        span.exit();
        let span = error_span!("push-events").entered();
        if this.position_send_timer.poll_tick(cx).is_ready() {
            this.send_events
                .send(Events::Position(*this.position))
                .trace_send_error();
        }
        span.exit();
        Poll::Pending
    }
}

fn play_index(mpv: &MpvStream, index: i64) -> Result<()> {
    mpv.playlist_play_index(index)
        .context("setting current playlist index")?;
    mpv.unpause().context("un pausing player")
}

fn stop(
    mpv: &MpvStream,
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    index: &mut Option<usize>,
    send_events: &broadcast::Sender<Events>,
) -> Result<()> {
    mpv.stop()?;
    *index = None;
    send_events.send(Events::Current(None)).trace_send_error();
    *playlist = Arc::new(Vec::new());
    send_events
        .send(Events::ReplacePlaylist {
            current: None,
            current_index: None,
            new_playlist: playlist.clone(),
        })
        .trace_send_error();
    assert_shadow_playlist_state(mpv, playlist)
}

fn remove_playlist_item(
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    mpv: &MpvStream,
    id: PlaylistItemId,
    send_events: &broadcast::Sender<Events>,
    cur_index: &mut Option<usize>,
) -> Result<()> {
    let index = index_of(playlist, id).ok_or_eyre("no such playlist item")?;
    mpv.playlist_remove_index(index.try_into().context("converting index to i64")?)
        .context("removing item from mpv playlist")?;
    let mut playlist_vec = Vec::clone(playlist);
    playlist_vec.remove(index);
    *playlist = Arc::new(playlist_vec);
    if *cur_index == Some(index) {
        *cur_index = None;
        send_events.send(Events::Current(None)).trace_send_error();
    }
    send_events
        .send(Events::RemovePlaylistItem {
            removed: id,
            new_playlist: playlist.clone(),
        })
        .trace_send_error();
    assert_shadow_playlist_state(mpv, playlist)
}

fn replace_playlist(
    mpv: &MpvStream,
    jellyfin: &JellyfinClient,
    playlist_id_gen: &mut PlaylistItemIdGen,
    playlist: &mut Arc<Vec<Arc<PlaylistItem>>>,
    items: Vec<MediaItem>,
    first: usize,
    send_events: &broadcast::Sender<Events>,
    index: &mut Option<usize>,
) -> Result<()> {
    if first >= items.len() {
        bail!("could not set playlist because first {first} is out of bounds.")
    }
    info!("replacing playlist with new list of length {}", items.len());
    mpv.playlist_clear()?;
    *index = None;
    send_events.send(Events::Current(None)).trace_send_error();
    *playlist = Arc::new(
        set_playlist(mpv, jellyfin, playlist_id_gen, items, first).context("replacing playlist")?,
    );
    mpv.playlist_play_index(first.try_into()?)?;
    assert_shadow_playlist_state(mpv, playlist)?;
    send_events
        .send(Events::ReplacePlaylist {
            current: playlist
                .get(first)
                .expect("current is missing from playlist")
                .id
                .into(),
            current_index: Some(first),
            new_playlist: playlist.clone(),
        })
        .trace_send_error();
    mpv.unpause()?;
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
    send_events: &broadcast::Sender<Events>,
) -> Result<()> {
    let uri = jellyfin.get_video_uri(&item)?.to_string();

    let index = if let Some(id) = after {
        index_of(playlist, id).ok_or_eyre("could not find this item id!")?
    } else {
        0
    };
    info!("inserting item at index {index}");
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
        MpvNodeMapRef::new(
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
    let mut playlist_vec = Vec::clone(playlist);
    playlist_vec.insert(index, Arc::new(PlaylistItem { item: *item, id }));
    *playlist = Arc::new(playlist_vec);
    assert_shadow_playlist_state(mpv, playlist)?;
    send_events
        .send(Events::AddPlaylistItem {
            after,
            index,
            new_playlist: playlist.clone(),
        })
        .trace_send_error();
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
