use std::marker::PhantomData;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde::Serialize;

use base64::{engine::general_purpose::URL_SAFE, Engine};
use tracing::{instrument, trace};

use crate::{
    err::JellyfinError, sha::Sha256, user::UserAuth, Auth, ClientInfo, JellyfinClient, KeyAuth,
    NoAuth,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUserNameReq<'a> {
    username: &'a str,
    pw: &'a str,
}
impl<Sha: Sha256> JellyfinClient<NoAuth, Sha> {
    pub fn auth_key(self, key: impl ToString) -> JellyfinClient<KeyAuth, Sha> {
        let key = key.to_string();
        let auth_header = make_key_auth_header::<Sha>(&key, &self.client_info, &self.device_name);
        JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: KeyAuth {
                access_key: key,
                header: auth_header,
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
        let client_id = make_user_client_id::<Sha>(username, &self.client_info, &self.device_name);
        let req = match self
            .client
            .post(format!("{}Users/AuthenticateByName", self.url))
            .json(&AuthUserNameReq {
                username: username.as_ref(),
                pw: password.as_ref(),
            })
            .header(
                AUTHORIZATION,
                make_auth_handshake_header(&self.client_info, &self.device_name, &client_id),
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
        let auth_header = make_user_auth_header(
            &auth.access_token,
            &self.client_info,
            &self.device_name,
            &client_id,
        );
        Ok(JellyfinClient {
            url: self.url,
            client: self.client,
            client_info: self.client_info,
            device_name: self.device_name,
            auth: Auth {
                user: auth.user,
                session_info: auth.session_info,
                access_token: auth.access_token,
                server_id: auth.server_id,
                header: auth_header,
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
fn make_user_auth_header(
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
fn make_user_client_id<Sha: Sha256>(
    user_name: &str,
    client_info: &ClientInfo,
    device_name: &str,
) -> String {
    let mut digest = Sha::new();
    digest.update(client_info.name.as_bytes());
    digest.update(client_info.version.as_bytes());
    digest.update(device_name.as_bytes());
    digest.update(user_name.as_bytes());
    let hash = digest.finalize();
    URL_SAFE.encode(hash)
}

#[instrument(skip_all)]
fn make_key_auth_header<Sha: Sha256>(
    key: &str,
    client_info: &ClientInfo,
    device_name: &str,
) -> HeaderValue {
    let mut val = r#"MediaBrowser Token=""#.to_string();
    val += key;
    val += r#"", Client=""#;
    val += &client_info.name;
    val += r#"", Version=""#;
    val += &client_info.version;
    val += r#"", Device=""#;
    URL_SAFE.encode_string(device_name.as_bytes(), &mut val);
    val += r#"", DeviceId=""#;
    make_key_client_id::<Sha>(client_info, device_name, &mut val);
    val.push('"');
    HeaderValue::try_from(val).expect("invalid client info for header value")
}

#[instrument(skip_all)]
fn make_key_client_id<Sha: Sha256>(client_info: &ClientInfo, device_name: &str, out: &mut String) {
    let mut digest = Sha::new();
    digest.update(client_info.name.as_bytes());
    digest.update(client_info.version.as_bytes());
    digest.update(device_name.as_bytes());
    let hash = digest.finalize();
    URL_SAFE.encode_string(hash, out);
}
