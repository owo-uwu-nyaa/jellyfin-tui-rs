use serde::Serialize;

use crate::{
    Authed, JellyfinClient, JellyfinVec, Result, connect::JsonResponse, items::MediaItem,
    request::RequestBuilderExt,
};

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

impl<Auth: Authed> JellyfinClient<Auth> {
    pub async fn get_episodes(
        &self,
        series_id: &str,
        query: &GetEpisodesQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(
            self.get(
                |prefix: &mut String| {
                    prefix.push_str("/Shows/");
                    prefix.push_str(series_id);
                    prefix.push_str("/Episodes");
                },
                query,
            )?
            .empty_body()?,
        )
        .await
    }

    pub async fn get_seasons(
        &self,
        series_id: &str,
        query: &GetSeasonsQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(
            self.get(
                |prefix: &mut String| {
                    prefix.push_str("/Shows/");
                    prefix.push_str(series_id);
                    prefix.push_str("/Seasons");
                },
                query,
            )?
            .empty_body()?,
        )
        .await
    }
}
