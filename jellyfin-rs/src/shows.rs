use serde::Serialize;

use crate::{items::MediaItem, sha::Sha256, Authed, JellyfinClient, JellyfinVec, JsonResponse, Result};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEpisodesQuery<'s> {
    pub user_id: Option<&'s str>,
    pub season: Option<u32>,
    pub season_id: Option<&'s str>,
    pub is_missing: Option<bool>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub enable_images: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'s str>,
    pub enable_user_data: Option<bool>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSeasonsQuery<'s> {
    pub user_id: Option<&'s str>,
    pub is_missing: Option<bool>,
    pub enable_images: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'s str>,
    pub enable_user_data: Option<bool>,
}

impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub async fn get_episodes(
        &self,
        series_id: &str,
        query: &GetEpisodesQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        let req = self
            .get(format!("{}Shows/{series_id}/Episodes", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }

    pub async fn get_seasons(
        &self,
        series_id: &str,
        query: &GetSeasonsQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        let req = self
            .get(format!("{}Shows/{series_id}/Seasons", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }
}
