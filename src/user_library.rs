use serde::Serialize;
use tracing::instrument;

use crate::{items::MediaItem, sha::Sha256, Authed, JellyfinClient};

use super::err::Result;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLatestQuery<'a> {
    pub user_id: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub fields: Option<&'a str>,
    pub include_item_types: Option<&'a str>,
    pub is_played: Option<bool>,
    pub enable_images: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'a str>,
    pub enable_user_data: Option<bool>,
    pub limit: Option<u32>,
    pub group_items: Option<bool>,
}

impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    #[instrument(skip(self))]
    pub async fn get_user_library_latest_media(
        &self,
        query: &GetLatestQuery<'_>,
    ) -> Result<Vec<MediaItem>> {
        let req = self
            .get(format!("{}Items/Latest", self.url))
            .query(query)
            .send()
            .await?;
        Ok(req.json().await?)
    }
}
