use std::{fmt::Display, future::Future};

use bytes::Bytes;
use reqwest::Client;
use serde::Serialize;

use crate::{items::ImageType, sha::ShaImpl, AuthStatus, JellyfinClient, Result};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetImageQuery<'s> {
    pub tag: Option<&'s str>,
    pub format: Option<&'s str>,
}

pub struct GetImage {
    client: Client,
    url: String,
}

impl GetImage {
    pub async fn get(self, query: &GetImageQuery<'_>) -> Result<Bytes> {
        Ok(self
            .client
            .get(self.url)
            .query(query)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    }
}

impl<Auth: AuthStatus, Sha: ShaImpl> JellyfinClient<Auth, Sha> {
    pub fn prepare_get_image(&self, item_id: impl Display, image_type: ImageType) -> GetImage {
        GetImage {
            client: self.client.clone(),
            url: format!("{}Items/{item_id}/Images/{image_type}", self.url),
        }
    }
    pub fn get_image<'a>(
        &'a self,
        item_id: impl Display,
        image_type: ImageType,
        query: &'a GetImageQuery<'_>,
    ) -> impl Future<Output = Result<Bytes>> + Send + Sync + 'a {
        self.prepare_get_image(item_id, image_type).get(query)
    }
}
