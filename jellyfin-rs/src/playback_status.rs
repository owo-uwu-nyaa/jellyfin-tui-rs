use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use crate::{Auth, JellyfinClient, Result, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct PlayingBody<'s> {
    item_id: &'s str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ProgressBody<'s> {
    pub item_id: &'s str,
    pub position_ticks: u64,
}

pub struct SetPlaying {
    inner: RequestBuilder,
}
impl SetPlaying {
    pub async fn send(self, item_id: &str) -> Result<()> {
        self.inner
            .json(&PlayingBody { item_id })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

pub struct SetPlayingProgress {
    inner: RequestBuilder,
}
impl SetPlayingProgress {
    pub async fn send(self, body: &ProgressBody<'_>) -> Result<()> {
        self.inner.json(body).send().await?.error_for_status()?;
        Ok(())
    }
}

pub struct SetPlayingStopped {
    inner: RequestBuilder,
}
impl SetPlayingStopped {
    pub async fn send(self, body: &ProgressBody<'_>) -> Result<()> {
        self.inner.json(body).send().await?.error_for_status()?;
        Ok(())
    }
}

impl<Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub fn prepare_set_playing(&self) -> SetPlaying {
        SetPlaying {
            inner: self.post(format!("{}Sessions/Playing", self.url)),
        }
    }
    pub async fn set_playing(&self, item_id: &str) -> Result<()> {
        self.prepare_set_playing().send(item_id).await
    }

    pub fn prepare_set_playing_progress(&self) -> SetPlayingProgress {
        SetPlayingProgress {
            inner: self.post(format!("{}Sessions/Playing/Progress", self.url)),
        }
    }
    pub async fn set_playing_progress(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.prepare_set_playing_progress().send(body).await
    }

    pub fn prepare_set_playing_stopped(&self) -> SetPlayingProgress {
        SetPlayingProgress {
            inner: self.post(format!("{}Sessions/Playing/Stopped", self.url)),
        }
    }
    pub async fn set_playing_stopped(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.prepare_set_playing_stopped().send(body).await
    }
}
