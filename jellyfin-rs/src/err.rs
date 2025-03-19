use std::{error, fmt};

pub type Result<T> = std::result::Result<T, JellyfinError>;

pub enum JellyfinError {
    NetworkError(reqwest::Error),
    UrlParseError(url::ParseError),
    GetrandomError(getrandom::Error),
    WebsocketError(tokio_websockets::Error),
    JsonError(serde_json::Error),
    Jellyfin(&'static str),
}

impl fmt::Debug for JellyfinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JellyfinError::NetworkError(error) => fmt::Debug::fmt(error, f),
            JellyfinError::UrlParseError(parse_error) => fmt::Debug::fmt(parse_error, f),
            JellyfinError::GetrandomError(error) => fmt::Debug::fmt(error, f),
            JellyfinError::WebsocketError(error) => fmt::Debug::fmt(error, f),
            JellyfinError::JsonError(error) => fmt::Debug::fmt(error, f),
            JellyfinError::Jellyfin(reason) => f.write_str(reason),
        }
    }
}

impl fmt::Display for JellyfinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkError(v) => fmt::Display::fmt(v, f),
            Self::UrlParseError(v) => fmt::Display::fmt(v, f),
            Self::GetrandomError(v) => fmt::Display::fmt(v, f),
            Self::WebsocketError(v) => fmt::Display::fmt(v, f),
            Self::JsonError(v) => fmt::Display::fmt(v, f),
            Self::Jellyfin(reason) => f.write_str(reason),
        }
    }
}

impl error::Error for JellyfinError {}

impl From<reqwest::Error> for JellyfinError {
    fn from(value: reqwest::Error) -> Self {
        Self::NetworkError(value)
    }
}

impl From<url::ParseError> for JellyfinError {
    fn from(value: url::ParseError) -> Self {
        Self::UrlParseError(value)
    }
}

impl From<getrandom::Error> for JellyfinError {
    fn from(value: getrandom::Error) -> Self {
        Self::GetrandomError(value)
    }
}

impl From<tokio_websockets::Error> for JellyfinError {
    fn from(value: tokio_websockets::Error) -> Self {
        Self::WebsocketError(value)
    }
}

impl From<serde_json::Error> for JellyfinError {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonError(value)
    }
}
