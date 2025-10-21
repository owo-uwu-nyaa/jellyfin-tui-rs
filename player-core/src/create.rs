use std::{
    ffi::CString,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use color_eyre::{
    Result,
    eyre::{Context, OptionExt, eyre},
};
use jellyfin::{
    JellyfinClient,
    items::{ItemType, MediaItem},
};
use libmpv::{
    Mpv, MpvProfile,
    events::EventContextAsync,
    node::{BorrowingCPtr, BorrowingMpvNodeMap, ToNode},
};
use spawn::spawn_bare;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use crate::{
    PlayerHandle, PlayerState, PlaylistItem, PlaylistItemIdGen, mpv_stream::MpvStream,
    poll::PollState,
};

impl PlayerHandle {
    pub fn new(
        jellyfin: JellyfinClient,
        hwdec: &str,
        profile: MpvProfile,
        log_level: &str,
        stop: CancellationToken,
        minimized: bool,
    ) -> Result<Self> {
        let mpv = MpvStream::new(&jellyfin, hwdec, profile, log_level, minimized)?;
        let mut send_timer = tokio::time::interval(Duration::from_secs(1));
        send_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let playlist = Arc::new(Vec::new());
        let (c_send, c_recv) = tokio::sync::mpsc::unbounded_channel();
        let (s_send, s_recv) = tokio::sync::watch::channel(PlayerState {
            playlist: playlist.clone(),
            current: None,
            pause: false,
            position: 0.0,
            fullscreen: true,
            idle: true,
            minimized,
        });

        spawn_bare(
            PollState {
                idle: true,
                closed: false,
                mpv,
                commands: c_recv,
                send: s_send,
                send_timer,
                paused: false,
                position: 0.0,
                index: None,
                fullscreen: true,
                stop: stop.cancelled_owned(),
                jellyfin,
                playlist,
                playlist_id_gen: PlaylistItemIdGen::default(),
                minimized,
            }
            .instrument(),
        );

        Ok(Self {
            closed: Arc::new(AtomicBool::new(false)),
            send: c_send,
            state: s_recv,
        })
    }
}

pub fn set_playlist(
    mpv: &Mpv<EventContextAsync>,
    jellyfin: &JellyfinClient,
    id_gen: &mut PlaylistItemIdGen,
    items: Vec<MediaItem>,
    index: usize,
) -> Result<Vec<Arc<PlaylistItem>>> {
    let position = items[index]
        .user_data
        .as_ref()
        .ok_or_eyre("user data missing")?
        .playback_position_ticks
        / 10000000;

    for item in items[0..index].iter() {
        append(mpv, jellyfin, item)?
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
        append(mpv, jellyfin, item)?
    }
    debug!("later files added");
    Ok(items
        .into_iter()
        .map(|item| {
            Arc::new(PlaylistItem {
                item,
                id: id_gen.next(),
            })
        })
        .collect())
}

#[instrument(skip_all)]
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
