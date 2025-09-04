use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    Authed, JellyfinClient, Result,
    connect::Connection,
    request::{NoQuery, PathBuilder, RequestBuilderExt},
};

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
    pub is_paused: bool,
}

struct Prepared {
    conn: Arc<Connection>,
    req: http::request::Builder,
}

impl Prepared {
    fn new<Auth: Authed>(client: &JellyfinClient<Auth>, uri: impl PathBuilder) -> Result<Self> {
        let req = client.post(uri, NoQuery)?;
        Ok(Prepared {
            conn: client.connection.clone(),
            req,
        })
    }
    async fn send(self, val: &impl Serialize) -> Result<()> {
        self.conn.send_request(self.req.json_body(val)?).await?;
        Ok(())
    }
}

pub struct SetPlaying {
    inner: Prepared,
}
impl SetPlaying {
    pub async fn send(self, item_id: &str) -> Result<()> {
        self.inner.send(&PlayingBody { item_id }).await
    }
}

pub struct SetPlayingProgress {
    inner: Prepared,
}
impl SetPlayingProgress {
    pub async fn send(self, body: &ProgressBody<'_>) -> Result<()> {
        self.inner.send(body).await
    }
}

pub struct SetPlayingStopped {
    inner: Prepared,
}
impl SetPlayingStopped {
    pub async fn send(self, body: &ProgressBody<'_>) -> Result<()> {
        self.inner.send(body).await
    }
}

impl<Auth: Authed> JellyfinClient<Auth> {
    pub fn prepare_set_playing(&self) -> Result<SetPlaying> {
        Ok(SetPlaying {
            inner: Prepared::new(self, "/Sessions/Playing")?,
        })
    }
    pub async fn set_playing(&self, item_id: &str) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing", NoQuery)?
                .json_body(&PlayingBody { item_id })?,
        )
        .await?;
        Ok(())
    }

    pub fn prepare_set_playing_progress(&self) -> Result<SetPlayingProgress> {
        Ok(SetPlayingProgress {
            inner: Prepared::new(self, "/Sessions/Playing/Progress")?,
        })
    }
    pub async fn set_playing_progress(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing/Progress", NoQuery)?
                .json_body(body)?,
        )
        .await?;
        Ok(())
    }

    pub fn prepare_set_playing_stopped(&self) -> Result<SetPlayingProgress> {
        Ok(SetPlayingProgress {
            inner: Prepared::new(self, "/Sessions/Playing/Stopped")?,
        })
    }
    pub async fn set_playing_stopped(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing/Stopped", NoQuery)?
                .json_body(body)?,
        )
        .await?;
        Ok(())
    }
}
