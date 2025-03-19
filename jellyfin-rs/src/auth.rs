use std::marker::PhantomData;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde::Serialize;

use base64::{engine::general_purpose::URL_SAFE, Engine};
use tracing::{instrument, trace};

use crate::{
    err::JellyfinError,
    sha::{Sha256, ShaImpl},
    user::{User, UserAuth},
    Auth, ClientInfo, JellyfinClient, KeyAuth, NoAuth,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUserNameReq<'a> {
    username: &'a str,
    pw: &'a str,
}
impl<Sha: ShaImpl> JellyfinClient<NoAuth, Sha> {
    pub fn auth_key(self, key: String, user_name: impl AsRef<str>) -> JellyfinClient<KeyAuth, Sha> {
        let key = key.to_string();
        let device_id =
            make_user_client_id::<Sha>(user_name.as_ref(), &self.client_info, &self.device_name);
        let auth_header = make_auth_header(&key, &self.client_info, &self.device_name, &device_id);
        JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: KeyAuth {
                access_key: key,
                header: auth_header,
                device_id,
            },
            _phantom: PhantomData,
        }
    }

    #[instrument(skip_all)]
    pub async fn auth_user_name(
        self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<JellyfinClient<Auth, Sha>, (Self, JellyfinError)> {
        let username = username.as_ref();
        let device_id = make_user_client_id::<Sha>(username, &self.client_info, &self.device_name);
        let req = match self
            .client
            .post(format!("{}Users/AuthenticateByName", self.url))
            .json(&AuthUserNameReq {
                username,
                pw: password.as_ref(),
            })
            .header(
                AUTHORIZATION,
                make_auth_handshake_header(&self.client_info, &self.device_name, &device_id),
            )
            .send()
            .await
        {
            Ok(req) => req,
            Err(e) => return Err((self, e.into())),
        };
        let req = match req.error_for_status() {
            Ok(req) => req,
            Err(e) => return Err((self, e.into())),
        };
        let auth: UserAuth = match req.json().await {
            Ok(auth) => auth,
            Err(e) => return Err((self, e.into())),
        };
        let auth_header = make_auth_header(
            &auth.access_token,
            &self.client_info,
            &self.device_name,
            &device_id,
        );
        Ok(JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: Auth {
                user: auth.user,
                access_token: auth.access_token,
                header: auth_header,
                device_id,
            },
            _phantom: PhantomData,
        })
    }
}

impl<Sha: ShaImpl> JellyfinClient<KeyAuth, Sha> {
    pub async fn get_self(self) -> Result<JellyfinClient<Auth, Sha>, (Self, JellyfinError)> {
        let req = match self.get(format!("{}Users/Me", self.url)).send().await {
            Ok(v) => v,
            Err(e) => return Err((self, e.into())),
        };
        let req = match req.error_for_status() {
            Ok(v) => v,
            Err(e) => return Err((self, e.into())),
        };
        let user: User = match req.json().await {
            Ok(v) => v,
            Err(e) => return Err((self, e.into())),
        };
        Ok(JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: Auth {
                user,
                access_token: self.auth.access_key,
                header: self.auth.header,
                device_id: self.auth.device_id,
            },
            _phantom: PhantomData,
        })
    }
}

#[instrument(skip_all)]
fn make_auth_handshake_header(
    client_info: &ClientInfo,
    device_name: &str,
    device_id: &str,
) -> HeaderValue {
    let mut val = r#"MediaBrowser Client=""#.to_string();
    val += &client_info.name;
    val += r#"", Version=""#;
    val += &client_info.version;
    val += r#"", Device=""#;
    URL_SAFE.encode_string(device_name.as_bytes(), &mut val);
    val += r#"", DeviceId=""#;
    val += device_id;
    val.push('"');
    trace!("header value: {val}");
    HeaderValue::try_from(val).expect("invalid client info for header value")
}

#[instrument(skip_all)]
fn make_auth_header(
    access_token: &str,
    client_info: &ClientInfo,
    device_name: &str,
    device_id: &str,
) -> HeaderValue {
    let mut val = r#"MediaBrowser Token=""#.to_string();
    val += access_token;
    val += r#"", Client=""#;
    val += &client_info.name;
    val += r#"", Version=""#;
    val += &client_info.version;
    val += r#"", Device=""#;
    URL_SAFE.encode_string(device_name.as_bytes(), &mut val);
    val += r#"", DeviceId=""#;
    val += device_id;
    val.push('"');
    HeaderValue::try_from(val).expect("invalid client info for header value")
}

#[instrument(skip_all)]
fn make_user_client_id<Sha: ShaImpl>(
    user_name: &str,
    client_info: &ClientInfo,
    device_name: &str,
) -> String {
    let mut digest = <Sha::S256 as Sha256>::new();
    digest.update(client_info.name.as_bytes());
    digest.update(client_info.version.as_bytes());
    digest.update(device_name.as_bytes());
    digest.update(user_name.as_bytes());
    let hash = digest.finalize();
    URL_SAFE.encode(hash)
}
