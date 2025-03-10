use serde::Serialize;

use crate::{items::MediaItem, sha::Sha256, Authed, JellyfinClient, JellyfinVec, JsonResponse};

#[derive(Debug, Default, Clone, Serialize)]
pub struct GetPlaylistItemsQuery<'s>{
    pub user_id: Option<&'s str>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub enable_images: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'s str>,
    pub enable_user_data: Option<bool>,
}


impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub async fn get_playlist_items(
        &self, playlist_id: &str, query: &GetPlaylistItemsQuery<'_>
    )->crate::Result<JsonResponse<JellyfinVec<MediaItem>>>{
        let req = self
            .get(format!("{}Playlists/{playlist_id}/Items", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }
}
