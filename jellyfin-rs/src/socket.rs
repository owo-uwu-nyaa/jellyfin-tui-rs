use std::{pin::Pin, task::Poll, time::Duration};

use base64::{prelude::BASE64_STANDARD, Engine};
use futures_core::{FusedStream, Stream};
use futures_sink::Sink;
use reqwest::{
    header::{CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    StatusCode, Upgraded,
};
use serde::{Deserialize, Serialize};
use tokio::time::{interval_at, Instant, Interval, MissedTickBehavior};
use tokio_websockets::{Message, WebSocketStream};

use crate::{
    err::JellyfinError,
    sha::{Sha1, ShaImpl},
    Auth, JellyfinClient, Result,
};

pub struct JellyfinWebSocket {
    socket: WebSocketStream<Upgraded>,
    send_keep_alive: bool,
    keep_alive_timer: Interval,
    closed: bool,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "MessageType")]
pub enum JellyfinMessage {
    KeepAlive,
    #[serde(skip_deserializing)]
    Binary(Vec<u8>),
    #[serde(untagged)]
    #[serde(rename_all = "PascalCase")]
    Unknown {
        message_type: String,
        data: serde_json::Value,
    },
}

impl FusedStream for JellyfinWebSocket {
    fn is_terminated(&self) -> bool {
        self.closed
    }
}

impl Stream for JellyfinWebSocket {
    type Item = Result<JellyfinMessage>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.closed {
            return Poll::Ready(None);
        }
        if !self.send_keep_alive && self.keep_alive_timer.poll_tick(cx).is_ready() {
            self.send_keep_alive = true;
        }
        if self.send_keep_alive {
            match Pin::new(&mut self.socket).poll_ready(cx) {
                Poll::Pending => {}
                Poll::Ready(Err(e)) => {
                    self.closed = true;
                    return Poll::Ready(Some(Err(e.into())));
                }
                Poll::Ready(Ok(())) => {
                    self.send_keep_alive = false;
                    if let Err(e) = Pin::new(&mut self.socket)
                        .start_send(Message::text("{\"MessageType\":\"KeepAlive\"}"))
                    {
                        return Poll::Ready(Some(Err(e.into())));
                    }
                }
            }
        }
        loop {
            break match Pin::new(&mut self.socket).poll_next(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(None) => {
                    self.closed = true;
                    Poll::Ready(None)
                }
                Poll::Ready(Some(Err(e))) => {
                    self.closed = true;
                    Poll::Ready(Some(Err(e.into())))
                }
                Poll::Ready(Some(Ok(message))) => {
                    if message.is_ping() || message.is_pong() {
                        continue;
                    } else if let Some(message) = message.as_text() {
                        match serde_json::from_str(message) {
                            Ok(message) => Poll::Ready(Some(Ok(message))),
                            Err(e) => {
                                self.closed = true;
                                Poll::Ready(Some(Err(e.into())))
                            }
                        }
                    } else {
                        if message.as_payload().is_empty() {
                            continue;
                        } else {
                            Poll::Ready(Some(Ok(JellyfinMessage::Binary(
                                message.as_payload().to_vec(),
                            ))))
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
struct SocketQuery<'s> {
    api_key: &'s str,
    deviceid: &'s str,
}

impl<Sha: ShaImpl> JellyfinClient<Auth, Sha> {
    pub async fn get_socket(&self) -> Result<JellyfinWebSocket> {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce)?;
        let nonce = BASE64_STANDARD.encode(nonce);
        let query = SocketQuery {
            api_key: &self.auth.access_token,
            deviceid: &self.auth.device_id,
        };
        let res = self
            .client
            .get(format!("{}socket", self.url))
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
        let accept = accept.finalize();
        let accept = BASE64_STANDARD.encode(accept);
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
        let socket = tokio_websockets::ClientBuilder::new().take_over(res.upgrade().await?);
        let keep_alive_interval = Duration::from_secs(30);
        let mut keep_alive_timer =
            interval_at(Instant::now() + keep_alive_interval, keep_alive_interval);
        keep_alive_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Ok(JellyfinWebSocket {
            socket,
            send_keep_alive: false,
            keep_alive_timer,
            closed: false,
        })
    }
}
