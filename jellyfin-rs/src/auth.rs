use std::sync::Arc;

use aws_lc_rs::digest;
use http::{header::AUTHORIZATION, HeaderValue};
use serde::Serialize;

use base64::{engine::general_purpose::URL_SAFE, Engine};
use tracing::{instrument, trace};

use crate::{
    client_with_auth,
    request::{NoQuery, RequestBuilderExt},
    user::{User, UserAuth},
    Auth, AuthStatus, ClientInfo, ClientInner, JellyfinClient, KeyAuth, NoAuth,
};

use std::result::Result as StdResult;

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUserNameReq<'a> {
    username: &'a str,
    pw: &'a str,
}
impl JellyfinClient<NoAuth> {
    pub fn auth_key(
        self,
        key: String,
        user_name: impl AsRef<str>,
    ) -> JellyfinClient<KeyAuth> {
        let key = key.to_string();
        let device_id = make_user_client_id(
            user_name.as_ref(),
            &self.inner.client_info,
            &self.inner.device_name,
        );
        let auth_header = make_auth_header(
            &key,
            &self.inner.client_info,
            &self.inner.device_name,
            &device_id,
        );
        client_with_auth(
            self,
            KeyAuth {
                access_key: key,
                header: auth_header,
                device_id,
            },
        )
    }

    #[instrument(skip_all)]
    pub async fn auth_user_name(
        self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> StdResult<JellyfinClient<Auth>, (Self, color_eyre::Report)> {
        let username = username.as_ref();
        let device_id =
            make_user_client_id(username, &self.inner.client_info, &self.inner.device_name);
        let auth: StdResult<UserAuth, color_eyre::Report> = async {
            self.send_request_json(
                self.post("/Users/AuthenticateByName", NoQuery)?
                    .header(
                        AUTHORIZATION,
                        make_auth_handshake_header(
                            &self.inner.client_info,
                            &self.inner.device_name,
                            &device_id,
                        ),
                    )
                    .json_body(&AuthUserNameReq {
                        username,
                        pw: password.as_ref(),
                    })?,
            )
            .await?
            .deserialize()
            .await
        }
        .await;
        let auth = match auth {
            Ok(v) => v,
            Err(e) => return Err((self, e)),
        };
        let auth_header = make_auth_header(
            &auth.access_token,
            &self.inner.client_info,
            &self.inner.device_name,
            &device_id,
        );

        let auth = Auth {
            user: auth.user,
            access_token: auth.access_token,
            header: auth_header,
            device_id,
        };
        Ok(make_auth_or_return(self, auth))
    }
}

fn make_auth_or_return<Auth1: AuthStatus, Auth2: AuthStatus>(
    this: JellyfinClient<Auth1>,
    auth: Auth2,
) -> JellyfinClient<Auth2> {
    let inner = match Arc::try_unwrap(this.inner) {
        Ok(client) => ClientInner {
            uri_base: client.uri_base,
            host_header: client.host_header,
            connection: client.connection,
            device_name: client.device_name,
            client_info: client.client_info,
            auth,
        },
        Err(client) => ClientInner {
            host_header: client.host_header.clone(),
            uri_base: client.uri_base.clone(),
            connection: client.connection.clone_new(),
            client_info: client.client_info.clone(),
            device_name: client.device_name.clone(),
            auth,
        },
    };
    JellyfinClient {
        inner: Arc::new(inner),
    }
}

impl JellyfinClient<KeyAuth> {
    pub async fn get_self(self) -> StdResult<JellyfinClient<Auth>, (Self, color_eyre::Report)> {
        let user = async {
            self.send_request_json(self.get("/Users/Me", NoQuery)?.empty_body()?)
                .await?
                .deserialize()
                .await
        };
        let user: User = match user.await {
            Ok(v) => v,
            Err(e) => return Err((self, e)),
        };

        let auth = Auth {
            user,
            access_token: self.inner.auth.access_key.clone(),
            header: self.inner.auth.header.clone(),
            device_id: self.inner.auth.device_id.clone(),
        };
        Ok(make_auth_or_return(self, auth))
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
fn make_user_client_id(user_name: &str, client_info: &ClientInfo, device_name: &str) -> String {
    let mut digest = digest::Context::new(&digest::SHA256);
    digest.update(client_info.name.as_bytes());
    digest.update(client_info.version.as_bytes());
    digest.update(device_name.as_bytes());
    digest.update(user_name.as_bytes());
    let hash = digest.finish();
    URL_SAFE.encode(hash)
}
