use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    Authed, JellyfinClient, Result,
    request::{NoQuery, RequestBuilderExt},
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
impl<Auth: Authed> JellyfinClient<Auth> {
    #[instrument(skip(self))]
    pub async fn set_playing(&self, item_id: &str) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing", NoQuery)?
                .json_body(&PlayingBody { item_id })?,
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn set_playing_progress(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing/Progress", NoQuery)?
                .json_body(body)?,
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn set_playing_stopped(&self, body: &ProgressBody<'_>) -> Result<()> {
        self.send_request(
            self.post("/Sessions/Playing/Stopped", NoQuery)?
                .json_body(body)?,
        )
        .await?;
        Ok(())
    }
}
