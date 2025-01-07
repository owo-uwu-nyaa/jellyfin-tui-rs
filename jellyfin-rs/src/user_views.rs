use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{items::ImageType, sha::Sha256, Auth, JellyfinClient, Result};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetUserViewsQuery<'s> {
    pub user_id: Option<&'s str>,
    pub include_external_content: Option<bool>,
    pub preset_views: Option<&'s str>,
    pub include_hidden: Option<bool>,
}

impl<Sha: Sha256> JellyfinClient<Auth, Sha> {
    #[instrument(skip(self))]
    pub async fn get_user_views(&self, query: &GetUserViewsQuery<'_>) -> Result<UserViews> {
        let req = self
            .get(format!("{}UserViews", self.url))
            .query(&query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct UserViews {
    pub items: Vec<UserView>,
    pub start_index: u32,
    pub total_record_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserViewType {
    CollectionFolder,
    UserView,
}
