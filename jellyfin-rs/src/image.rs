use bytes::Bytes;
use serde::Serialize;

use crate::{AuthStatus, JellyfinClient, Result, items::ImageType, request::RequestBuilderExt};

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetImageQuery<'s> {
    pub tag: Option<&'s str>,
    pub format: Option<&'s str>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
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
