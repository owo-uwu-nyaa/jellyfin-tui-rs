use thiserror::Error;

pub type Result<T> = std::result::Result<T, JellyfinError>;

#[derive(Debug, Error)]
pub enum JellyfinError {
    #[error("error in network request")]
    NetworkError(#[from] reqwest::Error),
    #[error("error parsing url")]
    UrlParseError(#[from] url::ParseError),
    #[error("error generating random data")]
    GetrandomError(getrandom::Error),
    #[error("error in websocket")]
    WebsocketError(#[from] tokio_websockets::Error),
    #[error("error in json serialization")]
    JsonError(#[from] serde_json::Error),
    #[error("{}",.0)]
    Jellyfin(&'static str),
}

impl From<getrandom::Error> for JellyfinError {
    fn from(value: getrandom::Error) -> Self {
        Self::GetrandomError(value)
    }
}
