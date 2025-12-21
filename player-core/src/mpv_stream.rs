use std::{
    ffi::{CStr, CString},
    ops::Deref,
    task::{Poll, ready},
};

use color_eyre::eyre::{Context, Result};
use futures_util::Stream;
use jellyfin::JellyfinClient;
use libmpv::{
    Format, Mpv, MpvProfile,
    events::{
        Event, EventContextAsync, EventContextAsyncExt, EventContextExt, PropertyData, mpv_event_id,
    },
    node::{MpvNodeArrayRef, ToNode},
};
use tracing::{info, instrument, trace, warn};

use super::log::log_message;

#[derive(Debug)]
pub enum ObservedProperty {
    Position(f64),
    Idle(bool),
    Pause(bool),
    Fullscreen(bool),
    Minimized(bool),
    PlaylistPos(i64),
    Volume(i64),
    Speed(f64),
}

#[derive(Debug)]
pub enum ClientCommand {
    Stop,
}

#[derive(Debug)]
pub enum MpvEvent {
    PropertyChanged(ObservedProperty),
    Command(ClientCommand),
    Seek,
}

pub struct MpvStream {
    mpv: Mpv<EventContextAsync>,
}

impl Deref for MpvStream {
    type Target = Mpv<EventContextAsync>;
    fn deref(&self) -> &Self::Target {
        &self.mpv
    }
}

impl Stream for MpvStream {
    type Item = Result<MpvEvent>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            let event = match ready!(self.mpv.poll_wait_event(cx)).context("waiting for mpv events")
            {
                Err(e) => break Some(Err(e)),
                Ok(v) => v,
            };
            trace!(?event);
            match event {
                Event::LogMessage {
                    prefix,
                    level: _,
                    text,
                    log_level,
                } => log_message(prefix, log_level, text),
                Event::Shutdown => {
                    info!("shutdown request received");
                    break None;
                }
                Event::Seek => {
                    break Some(Ok(MpvEvent::Seek));
                }
                Event::PropertyChange {
                    name,
                    change,
                    reply_userdata,
                } => match (name, change, reply_userdata) {
                    ("time-pos", PropertyData::Double(pos), 1) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Position(
                            pos,
                        ))));
                    }
                    ("idle-active", PropertyData::Flag(idle), 2) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Idle(idle))));
                    }
                    ("pause", PropertyData::Flag(pause), 3) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Pause(
                            pause,
                        ))));
                    }
                    ("fullscreen", PropertyData::Flag(fullscreen), 4) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Fullscreen(
                            fullscreen,
                        ))));
                    }
                    ("window-minimized", PropertyData::Flag(minimized), 5) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Minimized(
                            minimized,
                        ))));
                    }
                    ("playlist-pos", PropertyData::Int64(pos), 6) => {
                        break Some(Ok(MpvEvent::PropertyChanged(
                            ObservedProperty::PlaylistPos(pos),
                        )));
                    }
                    ("speed", PropertyData::Double(speed), 7) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Speed(
                            speed,
                        ))));
                    }
                    ("volume", PropertyData::Int64(volume), 8) => {
                        break Some(Ok(MpvEvent::PropertyChanged(ObservedProperty::Volume(
                            volume,
                        ))));
                    }
                    (name, val, id) => {
                        warn!(name, ?val, id, "received unrequested property change event");
                    }
                },
                Event::ClientMessage(message) => {
                    let message: Vec<_> = message.into_iter().map(CStr::to_bytes).collect();
                    match message.as_slice() {
                        &[b"stop-player"] => {
                            break Some(Ok(MpvEvent::Command(ClientCommand::Stop)));
                        }
                        message => {
                            warn!(?message, "received unknown client message");
                        }
                    }
                }
                _ => {}
            }
        })
    }
}

impl MpvStream {
    #[instrument(skip_all)]
    pub fn new(
        jellyfin: &JellyfinClient,
        hwdec: &str,
        profile: MpvProfile,
        log_level: &str,
        minimized: bool,
    ) -> Result<Self> {
        let mpv = Mpv::with_initializer(|mpv| -> Result<()> {
            mpv.set_option(c"title", c"jellyfin-tui-player")?;
            mpv.set_option(c"fullscreen", true)?;
            mpv.set_option(c"window-minimized", minimized)?;
            mpv.set_option(c"drag-and-drop", false)?;
            mpv.set_option(c"osc", true)?;
            mpv.set_option(c"vo", c"gpu-next")?;
            mpv.set_option(c"terminal", false)?;
            let mut header = b"authorization: ".to_vec();
            header.extend_from_slice(jellyfin.get_auth().header.as_bytes());
            mpv.set_option(
                c"http-header-fields",
                &MpvNodeArrayRef::new(&[CString::new(header)
                    .context("converting auth header to cstr")?
                    .to_node()]),
            )?;
            mpv.set_option(c"input-default-bindings", true)?;
            mpv.set_option(c"input-vo-keyboard", true)?;
            mpv.set_option(
                c"hwdec",
                CString::new(hwdec)
                    .context("converting hwdec to cstr")?
                    .as_c_str(),
            )?;
            mpv.set_option(c"idle", c"yes")?;
            mpv.with_profile(profile)?;
            Ok(())
        })?
        .enable_async();
        mpv.set_log_level(&CString::new(log_level).context("converting log level to cstr")?)?;
        mpv.enable_event(mpv_event_id::PropertyChange)?;
        mpv.enable_event(mpv_event_id::LogMessage)?;
        mpv.enable_event(mpv_event_id::QueueOverflow)?;
        mpv.enable_event(mpv_event_id::ClientMessage)?;
        mpv.enable_event(mpv_event_id::Seek)?;
        mpv.observe_property("time-pos", Format::Double, 1)?;
        mpv.observe_property("idle-active", Format::Flag, 2)?;
        mpv.observe_property("pause", Format::Flag, 3)?;
        mpv.observe_property("fullscreen", Format::Flag, 4)?;
        mpv.observe_property("window-minimized", Format::Flag, 5)?;
        mpv.observe_property("playlist-pos", Format::Int64, 6)?;
        mpv.observe_property("speed", Format::Double, 7)?;
        mpv.observe_property("volume", Format::Int64, 8)?;
        mpv.command(&[
            c"keybind".to_node(),
            c"q".to_node(),
            stop_cmd(mpv.client_name()).to_node(),
            c"on quit stop the player instead".to_node(),
        ])?;
        info!("mpv initialized");
        Ok(Self { mpv })
    }
}

fn stop_cmd(name: &CStr) -> CString {
    let name = name.to_bytes();
    let first = b"script-message-to ";
    let end = c" stop-player".to_bytes_with_nul();
    let mut vec = Vec::with_capacity(first.len() + name.len() + end.len());
    vec.extend_from_slice(first);
    vec.extend_from_slice(name);
    vec.extend_from_slice(end);
    CString::from_vec_with_nul(vec).expect("constructed with null byte")
}
