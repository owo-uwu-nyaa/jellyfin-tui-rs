use std::{borrow::Cow, future::Future, marker::PhantomData};

pub use err::Result;
use reqwest::{
    header::{HeaderValue, AUTHORIZATION},
    Client, IntoUrl, RequestBuilder,
};
use sealed::AuthSealed;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha::Sha256;
use url::Url;
use user::User;

pub mod activity;
pub mod auth;
pub mod err;
pub mod image;
pub mod items;
pub mod playback_status;
pub mod playlist;
pub mod session;
pub mod sha;
pub mod shows;
pub mod user;
pub mod user_library;
pub mod user_views;
pub use reqwest;

#[derive(Debug, Clone)]
pub struct JellyfinClient<AuthS: AuthStatus = Auth, Sha: Sha256 = sha::Default> {
    url: Url,
    client: Client,
    client_info: ClientInfo,
    device_name: Cow<'static, str>,
    auth: AuthS,
    _phantom: PhantomData<Sha>,
}

pub struct NoAuth;
pub struct Auth {
    pub user: User,
    pub access_token: String,
    pub header: HeaderValue,
}

pub struct KeyAuth {
    pub access_key: String,
    pub header: HeaderValue,
}

mod sealed {
    use crate::{Auth, KeyAuth, NoAuth};
    pub trait AuthSealed {}
    impl AuthSealed for NoAuth {}
    impl AuthSealed for Auth {}
    impl AuthSealed for KeyAuth {}
}

pub trait AuthStatus: AuthSealed {}
impl AuthStatus for NoAuth {}
impl AuthStatus for Auth {}
impl AuthStatus for KeyAuth {}
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

impl<AuthS: AuthStatus, Sha: Sha256> JellyfinClient<AuthS, Sha> {
    /// Creates a new `JellyfinConnection`
    /// * `url` The base jellyfin server url, without a trailing "/"
    pub fn new(
        url: impl AsRef<str>,
        client_info: ClientInfo,
        device_name: impl Into<Cow<'static, str>>,
    ) -> err::Result<JellyfinClient<NoAuth, Sha>> {
        Ok(JellyfinClient {
            url: Url::parse(url.as_ref())?,
            client: Client::builder()
                .connector_layer(tower::limit::concurrency::ConcurrencyLimitLayer::new(2))
                .connection_verbose(true)
                .build()?,
            auth: NoAuth,
            client_info,
            device_name: device_name.into(),
            _phantom: PhantomData,
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
    ) -> err::Result<JellyfinClient<Auth, Sha>> {
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
    ) -> Result<JellyfinClient<KeyAuth, Sha>> {
        Ok(Self::new(url, client_info, device_name)?.auth_key(key, username))
    }

    pub fn get_auth(&self) -> &AuthS {
        &self.auth
    }
    pub fn get_base_url(&self) -> &Url {
        &self.url
    }
    pub fn get_client_info(&self) -> &ClientInfo {
        &self.client_info
    }
    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }
    pub fn get_http_client(&self) -> &Client {
        &self.client
    }
}

impl<Sha: Sha256> JellyfinClient<NoAuth, Sha> {
    pub fn get_base_url_mut(&mut self) -> &mut Url {
        &mut self.url
    }
}

impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    fn get(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client
            .get(url)
            .header(AUTHORIZATION, self.auth.header().clone())
    }
    fn post(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client
            .post(url)
            .header(AUTHORIZATION, self.auth.header().clone())
    }
    fn delete(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client
            .delete(url)
            .header(AUTHORIZATION, self.auth.header().clone())
    }
    pub fn without_auth(self) -> JellyfinClient<NoAuth, Sha> {
        JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: NoAuth,
            _phantom: PhantomData,
        }
    }
}

pub struct JsonResponse<T: DeserializeOwned> {
    response: reqwest::Response,
    deserialize: PhantomData<T>,
}

impl<T: DeserializeOwned> JsonResponse<T> {
    pub async fn deserialize(self) -> Result<T> {
        Ok(self.response.json().await?)
    }
    pub async fn deserialize_value(self) -> Result<serde_json::Value> {
        Ok(self.response.json().await?)
    }
    pub async fn deserialize_as<V: DeserializeOwned>(self) -> Result<V> {
        Ok(self.response.json().await?)
    }
}

impl<T: DeserializeOwned> From<reqwest::Response> for JsonResponse<T> {
    fn from(value: reqwest::Response) -> Self {
        Self {
            response: value,
            deserialize: PhantomData,
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
            if let Some(total) = total {
                if total as usize <= res.len() {
                    break;
                }
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
