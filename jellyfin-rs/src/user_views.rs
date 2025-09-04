use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    Authed, JellyfinClient, JellyfinVec, Result, connect::JsonResponse, items::ImageType,
    request::RequestBuilderExt,
};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetUserViewsQuery<'s> {
    pub user_id: Option<&'s str>,
    pub include_external_content: Option<bool>,
    pub preset_views: Option<&'s str>,
    pub include_hidden: Option<bool>,
}

impl<Auth: Authed> JellyfinClient<Auth> {
    #[instrument(skip(self))]
    pub async fn get_user_views(
        &self,
        query: &GetUserViewsQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<UserView>>> {
        self.send_request_json(self.get("/UserViews", query)?.empty_body()?)
            .await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct UserView {
    pub name: String,
    pub id: String,
    #[serde(rename = "Type")]
    pub view_type: UserViewType,
    pub image_tags: Option<HashMap<ImageType, String>>,
    pub sort_name: String,
    pub collection_type: CollectionType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserViewType {
    CollectionFolder,
    UserView,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CollectionType {
    Playlists,
    Movies,
    TvShows,
}
