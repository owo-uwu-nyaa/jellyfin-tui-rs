use bytes::Bytes;
use color_eyre::eyre::Context;
use http::Uri;
use serde::Serialize;

use crate::{
    AuthStatus, JellyfinClient, Result,
    items::{ImageType, MediaItem},
    request::RequestBuilderExt,
};

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

pub fn select_images(item: &MediaItem) -> impl Iterator<Item = (ImageType, &str)> {
    item.image_tags
        .iter()
        .flat_map(|map| map.iter())
        .map(|(image_type, tag)| (*image_type, tag.as_str()))
}

pub fn select_images_owned(item: MediaItem) -> impl Iterator<Item = (ImageType, String)> {
    item.image_tags
        .into_iter()
        .flat_map(|map| map.into_iter())
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
    pub fn get_image_uri(
        &self,
        item_id: &str,
        image_type: ImageType,
        query: &GetImageQuery<'_>,
    ) -> Result<Uri> {
        Uri::builder()
            .scheme(if self.tls() { "https" } else { "http" })
            .authority(self.authority().to_owned())
            .path_and_query(self.build_uri(
                |prefix: &mut String| {
                    prefix.push_str("/Items/");
                    prefix.push_str(item_id);
                    prefix.push_str("/Images/");
                    prefix.push_str(image_type.name());
                },
                query,
            )?)
            .build()
            .context("assembling image uri")
    }
}
