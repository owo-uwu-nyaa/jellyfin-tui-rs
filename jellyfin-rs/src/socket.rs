use std::{
    cmp::min,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{ready, Poll},
    time::Duration,
};

use base64::{prelude::BASE64_STANDARD, Engine};
use futures_core::Stream;
use futures_sink::Sink;
use pin_project_lite::pin_project;
use reqwest::{
    header::{CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    Client, StatusCode, Upgraded,
};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, sleep, Interval, Sleep};
use tokio_websockets::{Message, WebSocketStream};
use tracing::{debug, info, instrument, warn};

use crate::{
    err::JellyfinError,
    items::UserData,
    sha::{Sha1, ShaImpl},
    Auth, JellyfinClient, Result,
};

type SocketFuture = dyn Future<Output = Result<Upgraded>> + Send;

pin_project! {
    pub struct JellyfinWebSocket<Sha: ShaImpl = crate::sha::Default> {
        connect: ConnectInfo<Sha>,
        socket_future: Option<Pin<Box<SocketFuture>>>,
        #[pin]
        socket: Option<WebSocketStream<Upgraded>>,
        send_keep_alive: bool,
        keep_alive_timer: Option<Interval>,
        #[pin]
        reconnect: Option<Sleep>,
        backoff: bool,
        reconnect_sleep: Option<Duration>,
        closing: bool,
    }
}
#[derive(Debug)]
pub enum JellyfinMessage {
    Binary(Vec<u8>),
    RefreshProgress {
        item_id: String,
        progress: f64,
    },
    UserDataChanged {
        user_data_list: Vec<ChangedUserData>,
    },
    Unknown {
        message_type: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChangedUserData {
    pub item_id: String,
    pub key: String,
    #[serde(flatten)]
    pub user_data: UserData,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "MessageType")]
enum JellyfinMessageInternal {
    KeepAlive,
    #[serde(rename_all = "PascalCase")]
    ForceKeepAlive {
        data: u64,
    },
    #[serde(rename_all = "PascalCase")]
    RefreshProgress {
        item_id: String,
        progress: String,
    },
    #[serde(rename_all = "PascalCase")]
    UserDataChanged {
        user_data_list: Vec<ChangedUserData>,
    },
    #[serde(skip_deserializing)]
    Binary(Vec<u8>),
    #[serde(untagged)]
    #[serde(rename_all = "PascalCase")]
    Unknown {
        message_type: String,
        data: serde_json::Value,
    },
}

impl JellyfinMessageInternal {
    #[inline(always)]
    fn into_public<Sha: ShaImpl>(
        self,
        state: Pin<&mut JellyfinWebSocket<Sha>>,
    ) -> Option<JellyfinMessage> {
        match self {
            JellyfinMessageInternal::KeepAlive => None,
            JellyfinMessageInternal::ForceKeepAlive { data } => {
                let state = state.project();
                *state.keep_alive_timer = Some(interval(Duration::from_secs(data).div_f64(2.0)));
                *state.send_keep_alive = false;
                None
            }
            JellyfinMessageInternal::Binary(items) => Some(JellyfinMessage::Binary(items)),
            JellyfinMessageInternal::Unknown { message_type, data } => {
                Some(JellyfinMessage::Unknown { message_type, data })
            }
            JellyfinMessageInternal::RefreshProgress { item_id, progress } => {
                Some(JellyfinMessage::RefreshProgress {
                    item_id,
                    progress: progress
                        .parse()
                        .inspect_err(|e| warn!("Error parsing float: {e:?}"))
                        .ok()?,
                })
            }
            JellyfinMessageInternal::UserDataChanged { user_data_list } => {
                Some(JellyfinMessage::UserDataChanged { user_data_list })
            }
        }
    }
}

#[inline(always)]
fn poll_socket_connected<Sha: ShaImpl>(
    state: Pin<&mut JellyfinWebSocket<Sha>>,
    cx: &mut std::task::Context<'_>,
) -> Poll<()> {
    let mut state = state.project();
    if state.socket.is_none() {
        loop {
            if state.socket_future.is_none() {
                if *state.backoff {
                    if state.reconnect.is_none() {
                        //do exponential backoff to a maximum of 1 minute
                        let time = match state.reconnect_sleep {
                            None => Duration::from_secs(5),
                            Some(duration) => min((*duration) * 2, Duration::from_secs(60)),
                        };
                        *state.reconnect_sleep = Some(time);
                        info!("reconnecting in {} seconds", time.as_secs());
                        state.reconnect.set(Some(sleep(time)));
                    }
                    ready!(state
                        .reconnect
                        .as_mut()
                        .as_pin_mut()
                        .expect("filled by previous check")
                        .poll(cx));
                    state.reconnect.set(None);
                }
                *state.socket_future = Some(state.connect.new_connection());
            }
            match ready!(state
                .socket_future
                .as_mut()
                .expect("filled by previous check")
                .as_mut()
                .poll(cx))
            {
                Ok(stream) => {
                    state.socket.set(Some(
                        tokio_websockets::ClientBuilder::new().take_over(stream),
                    ));
                    *state.socket_future = None;
                    *state.backoff = false;
                    *state.reconnect_sleep = None;
                    *state.closing = false;
                    break;
                }
                Err(e) => {
                    warn!("error connecting web socket: {e:?}");
                    *state.socket_future = None;
                    *state.backoff = true;
                }
            }
        }
    }
    Poll::Ready(())
}

///closes the current stream
/// requires socket to be set
#[inline(always)]
fn poll_close<Sha: ShaImpl>(
    state: Pin<&mut JellyfinWebSocket<Sha>>,
    cx: &mut std::task::Context<'_>,
) -> Poll<()> {
    let mut state = state.project();
    let _ = ready!(state
        .socket
        .as_mut()
        .as_pin_mut()
        .expect("filled by previous function")
        .poll_close(cx));
    debug!("socket closed successfully");
    state.socket.set(None);
    *state.keep_alive_timer = None;
    Poll::Ready(())
}

/// send keep alive if needed
/// requires socket to be set
/// returns true if an error occurred and the parent loop should be restarted
#[inline(always)]
fn poll_keep_alive<Sha: ShaImpl>(
    state: Pin<&mut JellyfinWebSocket<Sha>>,
    cx: &mut std::task::Context<'_>,
) -> bool {
    let mut state = state.project();
    if !*state.send_keep_alive
        && state
            .keep_alive_timer
            .as_mut()
            .map(|i| i.poll_tick(cx).is_ready())
            .unwrap_or(false)
    {
        *state.send_keep_alive = true;
    }
    if *state.send_keep_alive {
        let mut socket = state
            .socket
            .as_mut()
            .as_pin_mut()
            .expect("set in previous function");
        match socket.as_mut().poll_ready(cx) {
            Poll::Pending => false,
            Poll::Ready(Err(e)) => {
                warn!("error waiting for socket to be ready: {e:?}");
                *state.closing = true;
                true
            }
            Poll::Ready(Ok(())) => {
                *state.send_keep_alive = false;
                if let Err(e) = socket.start_send(Message::text("{\"MessageType\":\"KeepAlive\"}"))
                {
                    warn!("error sending keep alive message: {e:?}");
                    *state.closing = true;
                    true
                } else {
                    false
                }
            }
        }
    } else {
        false
    }
}

#[inline(always)]
fn poll_message<Sha: ShaImpl>(
    state: Pin<&mut JellyfinWebSocket<Sha>>,
    cx: &mut std::task::Context<'_>,
) -> Poll<Option<Result<JellyfinMessageInternal>>> {
    let mut state = state.project();
    Poll::Ready(
        match ready!(state
            .socket
            .as_mut()
            .as_pin_mut()
            .expect("set in previous function")
            .poll_next(cx))
        {
            None => {
                *state.closing = true;
                None
            }
            Some(Err(e)) => {
                warn!("error receiving message: {e:?}");
                *state.closing = true;
                None
            }
            Some(Ok(message)) => {
                if message.is_ping() || message.is_pong() {
                    None
                } else if let Some(message) = message.as_text() {
                    match serde_json::from_str(message) {
                        Ok(message) => Some(Ok(message)),
                        Err(e) => Some(Err(e.into())),
                    }
                } else if message.as_payload().is_empty() {
                    None
                } else {
                    Some(Ok(JellyfinMessageInternal::Binary(
                        message.as_payload().to_vec(),
                    )))
                }
            }
        },
    )
}

impl<Sha: ShaImpl> Stream for JellyfinWebSocket<Sha> {
    type Item = Result<JellyfinMessage>;

    #[instrument(skip_all, name = "poll_jellyfin_web_socket")]
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            ready!(poll_socket_connected(self.as_mut(), cx));
            if self.closing {
                ready!(poll_close(self.as_mut(), cx));
                continue;
            }
            if poll_keep_alive(self.as_mut(), cx) {
                continue;
            }
            match ready!(poll_message(self.as_mut(), cx)) {
                Some(Ok(message)) => {
                    debug!("internal message: {message:#?}");
                    if let Some(message) = message.into_public(self.as_mut()) {
                        break Poll::Ready(Some(Ok(message)));
                    }
                }
                Some(Err(e)) => break Poll::Ready(Some(Err(e))),
                None => {}
            }
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
struct SocketQuery<'s> {
    api_key: &'s str,
    deviceid: &'s str,
}

struct ConnectInfo<Sha: ShaImpl> {
    url: String,
    client: Client,
    access_token: Arc<str>,
    deviceid: Arc<str>,
    _sha: PhantomData<Sha>,
}

impl<Sha: ShaImpl> ConnectInfo<Sha> {
    fn new_connection(&self) -> Pin<Box<SocketFuture>> {
        let request = self.client.get(&self.url);
        let access_token = self.access_token.clone();
        let deviceid = self.deviceid.clone();
        Box::pin(async move {
            let mut nonce = [0; 16];
            getrandom::fill(&mut nonce)?;
            let nonce = BASE64_STANDARD.encode(nonce);
            let query = SocketQuery {
                api_key: &access_token,
                deviceid: &deviceid,
            };
            let res = request
                .header(UPGRADE, "websocket")
                .header(CONNECTION, "Upgrade")
                .header(SEC_WEBSOCKET_KEY, &nonce)
                .header(SEC_WEBSOCKET_VERSION, "13")
                .query(&query)
                .send()
                .await?
                .error_for_status()?;
            if res.status() != StatusCode::SWITCHING_PROTOCOLS {
                return Err(JellyfinError::Jellyfin("wrong status code"));
            }
            let mut accept = <Sha::S1 as Sha1>::new();
            accept.update(nonce.as_bytes());
            accept.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
            let accept_bytes = accept.finalize();
            let accept = BASE64_STANDARD.encode(accept_bytes);
            if res
                .headers()
                .get(SEC_WEBSOCKET_ACCEPT)
                .ok_or(JellyfinError::Jellyfin(
                    "missing sec-websocket-accept header",
                ))?
                .as_bytes()
                != accept.as_bytes()
            {
                return Err(JellyfinError::Jellyfin(
                    "sec-websocket-accept has a wrong value",
                ));
            }
            Ok(res.upgrade().await?)
        })
    }
}

impl<Sha: ShaImpl> JellyfinClient<Auth, Sha> {
    pub fn get_socket(&self) -> JellyfinWebSocket<Sha> {
        let connect = ConnectInfo {
            url: format!("{}socket", self.url),
            client: self.client.clone(),
            access_token: Arc::from(self.auth.access_token.as_str()),
            deviceid: Arc::from(self.auth.device_id.as_str()),
            _sha: PhantomData,
        };
        JellyfinWebSocket {
            connect,
            socket_future: None,
            socket: None,
            send_keep_alive: false,
            keep_alive_timer: None,
            reconnect: None,
            backoff: false,
            reconnect_sleep: None,
            closing: false,
        }
    }
}
