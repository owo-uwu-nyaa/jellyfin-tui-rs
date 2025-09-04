use serde::Serialize;

use crate::{
    Authed, JellyfinClient, JellyfinVec, connect::JsonResponse, items::MediaItem,
    request::RequestBuilderExt,
};

#[derive(Debug, Default, Clone, Serialize)]
pub struct GetPlaylistItemsQuery<'s> {
    pub user_id: Option<&'s str>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub enable_images: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'s str>,
    pub enable_user_data: Option<bool>,
}

impl<Auth: Authed> JellyfinClient<Auth> {
    pub async fn get_playlist_items(
        &self,
        playlist_id: &str,
        query: &GetPlaylistItemsQuery<'_>,
    ) -> crate::Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(
            self.get(
                |prefix: &mut String| {
                    prefix.push_str("/Playlists/");
                    prefix.push_str(playlist_id);
                    prefix.push_str("/Items");
                },
                query,
            )?
            .empty_body()?,
        )
        .await
    }
}
