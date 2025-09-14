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
use tracing::{debug, instrument};

use crate::{PlayerHandle, PlayerRef, PlayerState, mpv_stream::MpvStream, poll::PollState};

pub(crate) fn player(
    jellyfin: &JellyfinClient,
    hwdec: &str,
    profile: MpvProfile,
    log_level: &str,
    items: Vec<MediaItem>,
    index: usize,
) -> Result<PlayerHandle> {
    let mpv = MpvStream::new(jellyfin, hwdec, profile, log_level)?;

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

    let items: Vec<_> = items.into_iter().map(Arc::new).collect();
    let (c_send, c_recv) = tokio::sync::mpsc::unbounded_channel();
    let (s_send, s_recv) = tokio::sync::watch::channel(PlayerState {
        index,
        current: items
            .get(index)
            .ok_or_eyre("accessing current media item")?
            .clone(),
        pause: false,
        position: 0.0,
    });

    spawn_bare(
        PollState {
            closed: false,
            started: false,
            mpv,
            commands: c_recv,
            send: s_send,
            send_timer,
            paused: false,
            position: 0.0,
            index,
            items,
            initial_index: index,
        }
        .instrument(),
    );

    Ok(PlayerHandle {
        inner: PlayerRef {
            closed: Arc::new(AtomicBool::new(false)),
            send: c_send,
            state: s_recv,
        },
    })
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
