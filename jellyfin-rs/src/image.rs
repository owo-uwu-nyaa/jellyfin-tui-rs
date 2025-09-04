use std::sync::Arc;

use bytes::Bytes;
use serde::Serialize;

use crate::{
    AuthStatus, JellyfinClient, Result, connect::Connection, items::ImageType,
    request::RequestBuilderExt,
};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetImageQuery<'s> {
    pub tag: Option<&'s str>,
    pub format: Option<&'s str>,
}

pub struct GetImage {
    connection: Arc<Connection>,
    req: http::Request<String>,
}

impl GetImage {
    pub async fn get(self) -> Result<Bytes> {
        Ok(self.connection.send_request(self.req).await?.0.into())
    }
}

fn image_req(
    client: &JellyfinClient<impl AuthStatus>,
    item_id: &str,
    image_type: ImageType,
    query: &GetImageQuery<'_>,
) -> Result<http::Request<String>> {
    client
        .get(
            |prefix: &mut String| {
                prefix.push_str("/Items/");
                prefix.push_str(item_id);
                prefix.push_str("/Images/");
                prefix.push_str(image_type.name());
            },
            query,
        )?
        .empty_body()
}

impl<Auth: AuthStatus> JellyfinClient<Auth> {
    pub fn prepare_get_image(
        &self,
        item_id: &str,
        image_type: ImageType,
        query: &GetImageQuery<'_>,
    ) -> Result<GetImage> {
        Ok(GetImage {
            connection: self.connection.clone(),
            req: image_req(self, item_id, image_type, query)?,
        })
    }
    pub async fn get_image(
        &self,
        item_id: &str,
        image_type: ImageType,
        query: &GetImageQuery<'_>,
    ) -> Result<Bytes> {
        Ok(self
            .send_request(image_req(self, item_id, image_type, query)?)
            .await?
            .0
            .into())
    }
}
