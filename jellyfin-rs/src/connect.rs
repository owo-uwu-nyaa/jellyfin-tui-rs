use std::{
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    net::IpAddr,
    ops::DerefMut,
    pin::pin,
    sync::Arc,
    task::{Poll, ready},
};

use bytes::{Bytes, BytesMut};
use color_eyre::{Section, SectionExt, eyre::eyre};
use futures_util::{FutureExt, future::poll_fn};
use http::{
    Request, Response,
    header::{CONTENT_LENGTH, CONTENT_TYPE},
    response::Parts,
    uri::Authority,
};
use hyper::body::{Body, Incoming};
use pin_project_lite::pin_project;
use serde::de::DeserializeOwned;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::Mutex,
};
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};
use tracing::{Instrument, error, error_span, instrument, warn};

use crate::Result;

pub struct Connection {
    authority: Authority,
    host: ServerName<'static>,
    port: u16,
    tls: bool,
    inner: Mutex<ConnectionInner>,
    general_config: TlsConnector,
    http1_config: TlsConnector,
}

impl Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("authority", &self.authority)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .finish()
    }
}

enum ConnectionInner {
    Disconnected,
    H2(hyper::client::conn::http2::SendRequest<String>),
    H1(hyper::client::conn::http1::SendRequest<String>),
}

impl Connection {
    pub fn clone_new(&self)->Self{
        Self{
            authority: self.authority.clone(),
            host: self.host.clone(),
            port: self.port,
            tls: self.tls,
            inner: Mutex::new(ConnectionInner::Disconnected),
            general_config: self.general_config.clone(),
            http1_config: self.http1_config.clone(),
        }
    }
    
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
    pub fn host(&self) -> &ServerName<'static> {
        &self.host
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn tls(&self) -> bool {
        self.tls
    }

    pub fn new(authority: Authority, tls: bool) -> Result<Self> {
        let host = ServerName::try_from(authority.host())?.to_owned();
        let port = authority.port_u16().unwrap_or(if tls { 443 } else { 80 });
        let mut cert_store = RootCertStore::empty();
        let certs = rustls_native_certs::load_native_certs();
        if let Some(e) = certs.errors.into_iter().next() {
            return Err(e.into());
        }
        for cert in certs.certs {
            cert_store.add(cert)?
        }
        let cert_store = Arc::new(cert_store);
        let http1_config = ClientConfig::builder()
            .with_root_certificates(cert_store)
            .with_no_client_auth();
        let mut general_config = http1_config.clone();
        general_config.alpn_protocols.push("h2".as_bytes().to_vec());
        general_config
            .alpn_protocols
            .push("http/1.1".as_bytes().to_vec());
        Ok(Connection {
            authority,
            host,
            port,
            tls,
            inner: Mutex::new(ConnectionInner::Disconnected),
            general_config: Arc::new(general_config).into(),
            http1_config: Arc::new(http1_config).into(),
        })
    }

    pub async fn http1_base_connection(&self) -> Result<MaybeTls> {
        let stream = get_stream(&self.host, self.port).await?;
        let stream = if self.tls {
            MaybeTls::Tcp {
                stream: self.http1_config.connect(self.host.clone(), stream).await?,
            }
        } else {
            MaybeTls::Plain { stream }
        };
        Ok(stream)
    }

    pub async fn send_request_json<T: DeserializeOwned>(
        &self,
        req: Request<String>,
    ) -> Result<JsonResponse<T>> {
        let (data, parts) = self.send_request(req).await?;
        if let Some(content_type) = parts.headers.get(CONTENT_TYPE)
            && content_type.to_str()?.contains("application/json")
        {
            Ok(JsonResponse::from(Bytes::from(data)))
        } else {
            Err(eyre!("Response does not have json CONTENT_TYPE"))
        }
    }

    #[instrument(skip_all)]
    pub async fn send_request(&self, req: Request<String>) -> Result<(BytesMut, Parts)> {
        loop {
            let mut state = self.inner.lock().await;
            let resp = loop {
                let inner = match state.deref_mut() {
                    ConnectionInner::Disconnected => {
                        let stream = get_stream(&self.host, self.port).await?;
                        if self.tls {
                            let stream = self
                                .general_config
                                .connect(self.host.clone(), stream)
                                .await?;
                            if let Some(b"h2") = stream.get_ref().1.alpn_protocol() {
                                let (send, con) = hyper::client::conn::http2::handshake(
                                    hyper_util::rt::TokioExecutor::new(),
                                    hyper_util::rt::TokioIo::new(stream),
                                )
                                .await?;
                                spawn_con(con);
                                ConnectionInner::H2(send)
                            } else {
                                let (send, con) = hyper::client::conn::http1::handshake(
                                    hyper_util::rt::TokioIo::new(stream),
                                )
                                .await?;
                                spawn_con(con);
                                ConnectionInner::H1(send)
                            }
                        } else {
                            let (send, con) = hyper::client::conn::http1::handshake(
                                hyper_util::rt::TokioIo::new(stream),
                            )
                            .await?;
                            spawn_con(con);
                            ConnectionInner::H1(send)
                        }
                    }
                    ConnectionInner::H2(send_request) => {
                        if let Err(e) = send_request.ready().await {
                            error!("error sending request: {e:?}");
                            ConnectionInner::Disconnected
                        } else {
                            break send_request.send_request(req.clone()).left_future();
                        }
                    }
                    ConnectionInner::H1(send_request) => {
                        if let Err(e) = send_request.ready().await {
                            error!("error sending request: {e:?}");
                            ConnectionInner::Disconnected
                        } else {
                            break send_request.send_request(req.clone()).right_future();
                        }
                    }
                };
                *state = inner;
            };
            drop(state);
            match resp.await {
                Ok(resp) => return recv_response(check_status(resp)?).await,
                Err(e) => {
                    if e.is_closed()
                        || e.is_incomplete_message()
                        || e.is_body_write_aborted()
                        || e.is_timeout()
                    {
                        warn!("retrying request")
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
    }
}

fn spawn_con(con: impl Future<Output = hyper::Result<()>> + Send + 'static) {
    tokio::spawn(
        async move {
            if let Err(e) = pin!(con).await {
                error!("connection error: {e:?}")
            }
        }
        .instrument(error_span!("jellyfin_connection")),
    );
}

async fn get_stream(host: &ServerName<'static>, port: u16) -> Result<TcpStream> {
    Ok(match host {
        ServerName::DnsName(dns_name) => TcpStream::connect((dns_name.as_ref(), port)).await?,
        ServerName::IpAddress(ip_addr) => {
            TcpStream::connect((IpAddr::from(ip_addr.to_owned()), port)).await?
        }
        _ => unimplemented!(),
    })
}

fn check_status<T>(response: Response<T>) -> Result<Response<T>> {
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        Err(eyre!("HTTP Error encountered: {status}"))
    } else {
        Ok(response)
    }
}

async fn recv_response(response: Response<Incoming>) -> Result<(BytesMut, Parts)> {
    let mut out = if let Some(length) = response.headers().get(CONTENT_LENGTH) {
        let length: usize = length.to_str()?.parse()?;
        BytesMut::with_capacity(length)
    } else {
        BytesMut::new()
    };
    let (parts, body) = response.into_parts();
    let mut parts = Some(parts);
    let mut body = pin!(body);
    poll_fn(move |cx| {
        while !body.is_end_stream() {
            if let Some(frame) = ready!(body.as_mut().poll_frame(cx)) {
                match frame {
                    Ok(frame) => {
                        if let Some(data) = frame.data_ref() {
                            out.extend_from_slice(data.as_ref())
                        }
                    }
                    Err(e) => return Poll::Ready(Err(e.into())),
                }
            } else {
                break;
            }
        }
        Poll::Ready(Ok((
            std::mem::replace(&mut out, BytesMut::new()),
            parts.take().expect("called twice"),
        )))
    })
    .await
}

pub struct JsonResponse<T: DeserializeOwned> {
    response: Bytes,
    deserialize: PhantomData<T>,
}

impl<T: DeserializeOwned> JsonResponse<T> {
    pub fn deserialize(self) -> impl Future<Output = Result<T>> {
        self.deserialize_as::<T>()
    }
    pub async fn deserialize_value(
        self,
    ) -> std::result::Result<serde_json::Value, serde_json::Error> {
        serde_json::from_slice(&self.response)
    }
    pub async fn deserialize_as<V: DeserializeOwned>(self) -> Result<V> {
        match serde_json::from_slice::<V>(&self.response) {
            Ok(v) => Ok(v),
            Err(first_try) => {
                let first_try = color_eyre::Report::from(first_try);
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&self.response) {
                    Err(first_try.wrap_err("Type mismatched with data").section(
                        serde_json::to_string_pretty(&v)
                            .expect("value should always be valid")
                            .header("Value:"),
                    ))
                } else {
                    Err(first_try.wrap_err("Invalid json syntax"))
                }
            }
        }
    }
}

impl<T: DeserializeOwned> From<Bytes> for JsonResponse<T> {
    fn from(value: Bytes) -> Self {
        JsonResponse {
            response: value,
            deserialize: PhantomData,
        }
    }
}

pin_project! {
    #[project = MaybeTlsProj]
    pub enum MaybeTls {
        Plain {
            #[pin]
            stream: TcpStream,
        },
        Tcp {
            #[pin]
            stream: TlsStream<TcpStream>,
        },
    }
}
impl AsyncRead for MaybeTls {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.project() {
            MaybeTlsProj::Plain { stream } => stream.poll_read(cx, buf),
            MaybeTlsProj::Tcp { stream } => stream.poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTls {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        match self.project() {
            MaybeTlsProj::Plain { stream } => stream.poll_write(cx, buf),
            MaybeTlsProj::Tcp { stream } => stream.poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        match self.project() {
            MaybeTlsProj::Plain { stream } => stream.poll_flush(cx),
            MaybeTlsProj::Tcp { stream } => stream.poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        match self.project() {
            MaybeTlsProj::Plain { stream } => stream.poll_shutdown(cx),
            MaybeTlsProj::Tcp { stream } => stream.poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        match self.project() {
            MaybeTlsProj::Plain { stream } => stream.poll_write_vectored(cx, bufs),
            MaybeTlsProj::Tcp { stream } => stream.poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            MaybeTls::Plain { stream } => stream.is_write_vectored(),
            MaybeTls::Tcp { stream } => stream.is_write_vectored(),
        }
    }
}
