use std::{borrow::Cow, future::Future, ops::Deref, sync::Arc};

use color_eyre::eyre::{OptionExt, eyre};
use connect::Connection;
pub use err::Result;
use http::{Uri, header::AUTHORIZATION};
use hyper::header::HeaderValue;
use sealed::AuthSealed;
use serde::{Deserialize, Serialize};
use user::User;

pub mod activity;
pub mod auth;
pub mod connect;
pub mod err;
pub mod image;
pub mod items;
pub mod playback_status;
pub mod playlist;
pub mod request;
pub mod session;
pub mod shows;
pub mod socket;
pub mod user;
pub mod user_library;
pub mod user_views;

#[derive(Debug, Clone)]
pub struct JellyfinClient<AuthS: AuthStatus = Auth> {
    host_header: HeaderValue,
    uri_base: String,
    connection: Arc<Connection>,
    client_info: ClientInfo,
    device_name: Cow<'static, str>,
    auth: AuthS,
}

impl<A: AuthStatus> Deref for JellyfinClient<A> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

pub struct NoAuth;
pub struct Auth {
    pub user: User,
    pub access_token: String,
    pub header: HeaderValue,
    pub device_id: String,
}

pub struct KeyAuth {
    pub access_key: String,
    pub header: HeaderValue,
    pub device_id: String,
}

mod sealed {
    use crate::{Auth, KeyAuth, NoAuth};
    pub trait AuthSealed {}
    impl AuthSealed for NoAuth {}
    impl AuthSealed for Auth {}
    impl AuthSealed for KeyAuth {}
}

pub trait AuthStatus: AuthSealed {
    fn add_auth_header(&self, builder: http::request::Builder) -> http::request::Builder;
}
impl AuthStatus for NoAuth {
    fn add_auth_header(&self, builder: http::request::Builder) -> http::request::Builder {
        builder
    }
}
impl AuthStatus for Auth {
    fn add_auth_header(&self, builder: http::request::Builder) -> http::request::Builder {
        builder.header(AUTHORIZATION, &self.header)
    }
}
impl AuthStatus for KeyAuth {
    fn add_auth_header(&self, builder: http::request::Builder) -> http::request::Builder {
        builder.header(AUTHORIZATION, &self.header)
    }
}
pub trait Authed: AuthStatus {
    fn token(&self) -> &str;
    fn header(&self) -> &HeaderValue;
}

impl Authed for Auth {
    fn token(&self) -> &str {
        &self.access_token
    }
    fn header(&self) -> &HeaderValue {
        &self.header
    }
}

impl Authed for KeyAuth {
    fn token(&self) -> &str {
        &self.access_key
    }
    fn header(&self) -> &HeaderValue {
        &self.header
    }
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub name: Cow<'static, str>,
    pub version: Cow<'static, str>,
}

impl<AuthS: AuthStatus> JellyfinClient<AuthS> {
    pub fn connection(&self) -> &Arc<Connection> {
        &self.connection
    }

    /// Creates a new `JellyfinConnection`
    /// * `url` The base jellyfin server url, without a trailing "/"
    pub fn new(
        uri: impl AsRef<str>,
        client_info: ClientInfo,
        device_name: impl Into<Cow<'static, str>>,
    ) -> err::Result<JellyfinClient<NoAuth>> {
        let uri = Uri::try_from(uri.as_ref())?.into_parts();
        let tls = match uri.scheme.as_ref().map(|s| s.as_str()) {
            None => return Err(eyre!("jellyfin uri has no scheme")),
            Some("http") => false,
            Some("https") => true,
            Some(val) => return Err(eyre!("unexpected jellyfin uri scheme {val}")),
        };
        let authority = uri.authority.ok_or_eyre("uri has no authority part")?;
        let host_header = HeaderValue::from_str(authority.as_str())?;
        let uri_base = uri
            .path_and_query
            .map(|path| path.path().trim_end_matches("/").to_string())
            .unwrap_or(String::new());
        Ok(JellyfinClient {
            uri_base,
            host_header,
            connection: Arc::new(Connection::new(authority, tls)?),
            auth: NoAuth,
            client_info,
            device_name: device_name.into(),
        })
    }

    /// Creates a new `JellyfinConnection` with auth
    /// * `url` The base jellyfin server url, without a traling "/"
    /// * `username` The username of the user to auth with
    /// * `password` The plain text password of the user to auth with
    pub async fn new_auth_name(
        url: impl AsRef<str>,
        client_info: ClientInfo,
        device_name: impl Into<Cow<'static, str>>,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> err::Result<JellyfinClient<Auth>> {
        Self::new(url, client_info, device_name)?
            .auth_user_name(username, password)
            .await
            .map_err(|(_, e)| e)
    }

    pub fn new_auth_key(
        url: impl AsRef<str>,
        client_info: ClientInfo,
        device_name: impl Into<Cow<'static, str>>,
        key: String,
        username: impl AsRef<str>,
    ) -> Result<JellyfinClient<KeyAuth>> {
        Ok(Self::new(url, client_info, device_name)?.auth_key(key, username))
    }

    pub fn get_auth(&self) -> &AuthS {
        &self.auth
    }
    pub fn get_base_uri(&self) -> &str {
        &self.uri_base
    }
    pub fn get_client_info(&self) -> &ClientInfo {
        &self.client_info
    }
    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }
}

impl<Auth: Authed> JellyfinClient<Auth> {
    pub fn without_auth(self) -> JellyfinClient<NoAuth> {
        JellyfinClient {
            uri_base: self.uri_base,
            host_header: self.host_header,
            connection: self.connection,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: NoAuth,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinVec<T> {
    pub items: Vec<T>,
    pub total_record_count: Option<u32>,
    pub start_index: u32,
}

impl<T> JellyfinVec<T> {
    pub async fn collect<I, F, E>(mut f: F) -> std::result::Result<Vec<T>, E>
    where
        F: FnMut(u32) -> I,
        I: Future<Output = std::result::Result<JellyfinVec<T>, E>>,
    {
        let initial = f(0).await?;
        let mut last_len = initial.items.len();
        let mut res = initial.items;
        let total = initial.total_record_count;
        loop {
            if let Some(total) = total
                && total as usize <= res.len()
            {
                break;
            }
            if last_len == 0 {
                break;
            }
            let mut next = f(res.len() as u32).await?;
            last_len = next.items.len();
            res.append(&mut next.items);
        }
        Ok(res)
    }
}
