use aws_lc_rs::digest;
use http::{HeaderValue, header::AUTHORIZATION};
use serde::Serialize;

use base64::{Engine, engine::general_purpose::URL_SAFE};
use tracing::{instrument, trace};

use crate::{
    Auth, ClientInfo, JellyfinClient, KeyAuth, NoAuth,
    request::{NoQuery, RequestBuilderExt},
    user::{User, UserAuth},
};

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUserNameReq<'a> {
    username: &'a str,
    pw: &'a str,
}
impl JellyfinClient<NoAuth> {
    pub fn auth_key(self, key: String, user_name: impl AsRef<str>) -> JellyfinClient<KeyAuth> {
        let key = key.to_string();
        let device_id =
            make_user_client_id(user_name.as_ref(), &self.client_info, &self.device_name);
        let auth_header = make_auth_header(&key, &self.client_info, &self.device_name, &device_id);
        JellyfinClient {
            client_info: self.client_info,
            device_name: self.device_name,
            auth: KeyAuth {
                access_key: key,
                header: auth_header,
                device_id,
            },
            host_header: self.host_header,
            uri_base: self.uri_base,
            connection: self.connection,
        }
    }

    #[instrument(skip_all)]
    pub async fn auth_user_name(
        self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<JellyfinClient<Auth>, (Self, color_eyre::Report)> {
        let username = username.as_ref();
        let device_id = make_user_client_id(username, &self.client_info, &self.device_name);
        let auth: Result<UserAuth, color_eyre::Report> = async {
            self.send_request_json(
                self.post("/Users/AuthenticateByName", NoQuery)?
                    .header(
                        AUTHORIZATION,
                        make_auth_handshake_header(
                            &self.client_info,
                            &self.device_name,
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
            &self.client_info,
            &self.device_name,
            &device_id,
        );
        Ok(JellyfinClient {
            client_info: self.client_info,
            device_name: self.device_name,
            auth: Auth {
                user: auth.user,
                access_token: auth.access_token,
                header: auth_header,
                device_id,
            },
            host_header: self.host_header,
            uri_base: self.uri_base,
            connection: self.connection,
        })
    }
}

impl JellyfinClient<KeyAuth> {
    pub async fn get_self(self) -> Result<JellyfinClient<Auth>, (Self, color_eyre::Report)> {
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
        Ok(JellyfinClient {
            client_info: self.client_info,
            device_name: self.device_name,
            auth: Auth {
                user,
                access_token: self.auth.access_key,
                header: self.auth.header,
                device_id: self.auth.device_id,
            },
            host_header: self.host_header,
            uri_base: self.uri_base,
            connection: self.connection,
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
fn make_user_client_id(user_name: &str, client_info: &ClientInfo, device_name: &str) -> String {
    let mut digest = digest::Context::new(&digest::SHA256);
    digest.update(client_info.name.as_bytes());
    digest.update(client_info.version.as_bytes());
    digest.update(device_name.as_bytes());
    digest.update(user_name.as_bytes());
    let hash = digest.finish();
    URL_SAFE.encode(hash)
}
