use std::{error, fmt};

pub type Result<T> = std::result::Result<T, JellyfinError>;

#[derive(Debug)]
pub enum JellyfinError {
    NetworkError(reqwest::Error),
    UrlParseError(url::ParseError),
}

impl fmt::Display for JellyfinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkError(v) => fmt::Display::fmt(v, f),
            Self::UrlParseError(v) => fmt::Display::fmt(v, f),
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
