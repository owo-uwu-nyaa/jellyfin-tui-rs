use std::{
    cmp::min,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Poll, ready},
    time::Duration,
};

use futures_core::Stream;
use futures_sink::Sink;
use http::Uri;
use pin_project_lite::pin_project;
use serde::{Deserialize, Serialize};
use tokio::time::{Interval, Sleep, interval, sleep};
use tokio_websockets::{Message, WebSocketStream};
use tracing::{debug, info};

use crate::{
    Auth, JellyfinClient, Result,
    connect::{Connection, MaybeTls},
    items::UserData,
};

type SocketFuture = dyn Future<Output = Result<WebSocketStream<MaybeTls>>> + Send;

pin_project! {
    #[project = SocketStateProj]
    enum SocketState{
        BackoffSleep{#[pin] sleep: Sleep, backoff_duration: Duration},
        Handshake{f:Pin<Box<SocketFuture>>,backoff_duration: Option<Duration>},
        Websocket{#[pin] socket:WebSocketStream<MaybeTls>, state: SocketHandlingState},
    }
}

enum SocketHandlingState {
    Close,
    Normal,
    WebsocketKeepAlive {
        keep_alive: Interval,
        send_now: bool,
    },
}

struct OpResult {
    state_change: Option<SocketState>,
    output: Option<Option<Result<JellyfinMessage>>>,
}

fn make_backoff(backoff_duration: Option<Duration>) -> SocketState {
    //do exponential backoff to a maximum of 1 minute
    let backoff_duration = match backoff_duration {
        None => Duration::from_secs(5),
        Some(duration) => min(duration * 2, Duration::from_secs(60)),
    };
    info!("reconnecting in {} seconds", backoff_duration.as_secs());
    SocketState::BackoffSleep {
        sleep: sleep(backoff_duration),
        backoff_duration,
    }
}

async fn make_websocket_future(
    builder: tokio_websockets::client::Builder<'static>,
    connect: Arc<Connection>,
) -> Result<WebSocketStream<MaybeTls>> {
    let conn = connect.http1_base_connection().await?;
    let (stream, _) = builder.connect_on(conn).await?;
    Ok(stream)
}

fn make_handshake(backoff_duration: Option<Duration>, connect: &ConnectInfo) -> SocketState {
    let builder = tokio_websockets::client::Builder::from_uri(connect.uri.clone());
    let connection = connect.connection.clone();
    let future = Box::pin(make_websocket_future(builder, connection));
    SocketState::Handshake {
        f: future,
        backoff_duration,
    }
}

fn make_websocket(socket: WebSocketStream<MaybeTls>) -> SocketState {
    SocketState::Websocket {
        socket,
        state: SocketHandlingState::Normal,
    }
}

fn poll_backoff_sleep(
    sleep: Pin<&mut Sleep>,
    backoff_duration: Duration,
    cx: &mut std::task::Context<'_>,
    connect: &ConnectInfo,
) -> Poll<OpResult> {
    ready!(sleep.poll(cx));
    Poll::Ready(OpResult {
        state_change: Some(make_handshake(Some(backoff_duration), connect)),
        output: None,
    })
}

fn poll_handshake(
    mut f: Pin<&mut SocketFuture>,
    backoff_duration: Option<Duration>,
    cx: &mut std::task::Context<'_>,
) -> Poll<OpResult> {
    match ready!(f.as_mut().poll(cx)) {
        Ok(socket) => Poll::Ready(OpResult {
            state_change: Some(make_websocket(socket)),
            output: None,
        }),
        Err(e) => Poll::Ready(OpResult {
            state_change: Some(make_backoff(backoff_duration)),
            output: Some(Some(Err(e))),
        }),
    }
}

struct WebsocketResult {
    parent: Option<OpResult>,
    socket: Option<SocketHandlingState>,
}

fn poll_websocket_close(
    socket: Pin<&mut WebSocketStream<MaybeTls>>,
    cx: &mut std::task::Context<'_>,
    connect: &ConnectInfo,
) -> Poll<WebsocketResult> {
    let res = ready!(socket.poll_close(cx));
    debug!("socket closed");
    let output = res.err().map(|e| Some(Err(e.into())));
    Poll::Ready(WebsocketResult {
        parent: Some(OpResult {
            state_change: Some(make_handshake(None, connect)),
            output,
        }),
        socket: None,
    })
}

fn poll_websocket_normal(
    mut socket: Pin<&mut WebSocketStream<MaybeTls>>,
    cx: &mut std::task::Context<'_>,
) -> Poll<WebsocketResult> {
    loop {
        match ready!(socket.as_mut().poll_next(cx)) {
            None => {
                debug!("websocket closed");
                return Poll::Ready(WebsocketResult {
                    parent: None,
                    socket: Some(SocketHandlingState::Close),
                });
            }
            Some(Err(e)) => {
                debug!("error in websocket: {e:?}");
                return Poll::Ready(WebsocketResult {
                    parent: Some(OpResult {
                        state_change: None,
                        output: Some(Some(Err(e.into()))),
                    }),
                    socket: Some(SocketHandlingState::Close),
                });
            }
            Some(Ok(message)) => {
                if message.is_ping() || message.is_pong() {
                } else if let Some(message) = message.as_text() {
                    match serde_json::from_str::<JellyfinMessageInternal>(message) {
                        Err(e) => {
                            return Poll::Ready(WebsocketResult {
                                parent: Some(OpResult {
                                    state_change: None,
                                    output: Some(Some(Err(e.into()))),
                                }),
                                socket: None,
                            });
                        }
                        Ok(JellyfinMessageInternal::KeepAlive) => {}
                        Ok(JellyfinMessageInternal::ForceKeepAlive { data }) => {
                            return Poll::Ready(WebsocketResult {
                                parent: None,
                                socket: Some(SocketHandlingState::WebsocketKeepAlive {
                                    keep_alive: interval(Duration::from_secs(data).div_f64(2.0)),
                                    send_now: true,
                                }),
                            });
                        }
                        Ok(JellyfinMessageInternal::Unknown { message_type, data }) => {
                            return Poll::Ready(WebsocketResult {
                                parent: Some(OpResult {
                                    state_change: None,
                                    output: Some(Some(Ok(JellyfinMessage::Unknown {
                                        message_type,
                                        data,
                                    }))),
                                }),
                                socket: None,
                            });
                        }
                        Ok(JellyfinMessageInternal::RefreshProgress { item_id, progress }) => {
                            let message = match progress.parse::<f64>() {
                                Ok(progress) => {
                                    Ok(JellyfinMessage::RefreshProgress { item_id, progress })
                                }
                                Err(e) => Err(e.into()),
                            };
                            return Poll::Ready(WebsocketResult {
                                parent: Some(OpResult {
                                    state_change: None,
                                    output: Some(Some(message)),
                                }),
                                socket: None,
                            });
                        }
                        Ok(JellyfinMessageInternal::UserDataChanged { user_data_list }) => {
                            return Poll::Ready(WebsocketResult {
                                parent: Some(OpResult {
                                    state_change: None,
                                    output: Some(Some(Ok(JellyfinMessage::UserDataChanged {
                                        user_data_list,
                                    }))),
                                }),
                                socket: None,
                            });
                        }
                    }
                } else if message.as_payload().is_empty() {
                } else {
                    return Poll::Ready(WebsocketResult {
                        parent: Some(OpResult {
                            state_change: None,
                            output: Some(Some(Ok(JellyfinMessage::Binary(
                                message.as_payload().to_vec(),
                            )))),
                        }),
                        socket: None,
                    });
                }
            }
        }
    }
}

fn poll_websocket_keep_alive(
    mut socket: Pin<&mut WebSocketStream<MaybeTls>>,
    keep_alive: &mut Interval,
    send_now: &mut bool,
    cx: &mut std::task::Context<'_>,
) -> Poll<WebsocketResult> {
    if keep_alive.poll_tick(cx).is_ready() {
        *send_now = true;
    }
    if *send_now {
        if let Err(e) = ready!(socket.as_mut().poll_ready(cx)) {
            debug!("error waiting for keep alive send to be ready");
            return Poll::Ready(WebsocketResult {
                parent: Some(OpResult {
                    state_change: None,
                    output: Some(Some(Err(e.into()))),
                }),
                socket: Some(SocketHandlingState::Close),
            });
        }
        if let Err(e) = socket
            .as_mut()
            .start_send(Message::text("{\"MessageType\":\"KeepAlive\"}"))
        {
            debug!("error sending keep alive");
            return Poll::Ready(WebsocketResult {
                parent: Some(OpResult {
                    state_change: None,
                    output: Some(Some(Err(e.into()))),
                }),
                socket: Some(SocketHandlingState::Close),
            });
        }
        *send_now = false;
    }
    poll_websocket_normal(socket, cx)
}

fn poll_websocket(
    mut socket: Pin<&mut WebSocketStream<MaybeTls>>,
    state: &mut SocketHandlingState,
    cx: &mut std::task::Context<'_>,
    connect: &ConnectInfo,
) -> Poll<OpResult> {
    loop {
        let res = match state {
            SocketHandlingState::Close => poll_websocket_close(socket.as_mut(), cx, connect),
            SocketHandlingState::Normal => poll_websocket_normal(socket.as_mut(), cx),
            SocketHandlingState::WebsocketKeepAlive {
                keep_alive,
                send_now,
            } => poll_websocket_keep_alive(socket.as_mut(), keep_alive, send_now, cx),
        };
        let res = ready!(res);
        if let Some(new_state) = res.socket {
            *state = new_state;
        }
        if let Some(parent) = res.parent {
            return Poll::Ready(parent);
        }
    }
}

impl SocketState {
    fn poll_state(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        connect: &ConnectInfo,
    ) -> Poll<Option<Result<JellyfinMessage>>> {
        loop {
            let res = match self.as_mut().project() {
                SocketStateProj::BackoffSleep {
                    sleep,
                    backoff_duration,
                } => poll_backoff_sleep(sleep, *backoff_duration, cx, connect),
                SocketStateProj::Handshake {
                    f,
                    backoff_duration,
                } => poll_handshake(f.as_mut(), *backoff_duration, cx),
                SocketStateProj::Websocket { socket, state } => {
                    poll_websocket(socket, state, cx, connect)
                }
            };
            let res = ready!(res);
            if let Some(state) = res.state_change {
                self.set(state);
            }
            if let Some(output) = res.output {
                return Poll::Ready(output);
            }
        }
    }
}

pin_project! {
    pub struct JellyfinWebSocket {
        connect: ConnectInfo,
        #[pin]
        state: SocketState,
    }
}

impl Stream for JellyfinWebSocket {
    type Item = Result<JellyfinMessage>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.state.poll_state(cx, this.connect)
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
    #[serde(untagged)]
    #[serde(rename_all = "PascalCase")]
    Unknown {
        message_type: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Default, Clone, Serialize)]
struct SocketQuery<'s> {
    api_key: &'s str,
    deviceid: &'s str,
}

struct ConnectInfo {
    uri: Uri,
    connection: Arc<Connection>,
}

impl JellyfinClient<Auth> {
    pub fn get_socket(&self) -> Result<JellyfinWebSocket> {
        let uri = http::uri::Builder::new()
            .scheme(if self.tls() { "wss" } else { "ws" })
            .authority(self.connection.authority().clone())
            .path_and_query(self.build_path(
                "/socket",
                SocketQuery {
                    api_key: &self.auth.access_token,
                    deviceid: &self.auth.device_id,
                },
            )?)
            .build()?;

        let connect = ConnectInfo {
            uri,
            connection: self.connection.clone(),
        };
        let state = make_handshake(None, &connect);
        Ok(JellyfinWebSocket { connect, state })
    }
}
