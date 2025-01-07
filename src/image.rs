use std::fmt::Display;

use bytes::Bytes;
use serde::Serialize;

use crate::{items::ImageType, sha::Sha256, AuthStatus, JellyfinClient, Result};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetImageQuery<'s> {
    pub tag: Option<&'s str>,
    pub format: Option<&'s str>,
}

impl<Auth: AuthStatus, Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub async fn get_image(
        &self,
        item_id: impl Display,
        image_type: ImageType,
        query: &GetImageQuery<'_>,
    ) -> Result<Bytes> {
        Ok(self
            .client
            .get(format!("{}Items/{item_id}/Images/{image_type}", self.url))
            .query(query)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    }
}
