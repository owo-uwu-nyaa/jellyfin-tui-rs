use std::collections::HashMap;
use std::fmt::Display;

use crate::{sha::ShaImpl, Auth, JellyfinClient, Result};
use crate::{JellyfinVec, JsonResponse};
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdQuery<'a> {
    pub user_id: Option<&'a str>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetItemsQuery<'a> {
    pub user_id: Option<&'a str>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub parent_id: Option<&'a str>,
    pub exclude_item_types: Option<&'a str>,
    pub include_item_types: Option<&'a str>,
    pub enable_images: Option<bool>,
    pub enable_image_types: Option<&'a str>,
    pub image_type_limit: Option<u32>,
    pub enable_user_data: Option<bool>,
    pub fields: Option<&'a str>,
    pub sort_by: Option<&'a str>,
    pub recursive: Option<bool>,
    pub sort_order: Option<&'a str>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResumeQuery<'a> {
    pub user_id: Option<&'a str>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub search_term: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub fields: Option<&'a str>,
    pub media_types: Option<&'a str>,
    pub enable_user_data: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'a str>,
    pub exclude_item_types: Option<&'a str>,
    pub include_item_types: Option<&'a str>,
    pub enable_total_record_count: Option<bool>,
    pub enable_images: Option<bool>,
    pub exclude_active_sessions: Option<bool>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNextUpQuery<'a> {
    pub user_id: Option<&'a str>,
    pub start_index: Option<u32>,
    pub limit: Option<u32>,
    pub parent_id: Option<&'a str>,
    pub series_id: Option<&'a str>,
    pub fields: Option<&'a str>,
    pub enable_user_data: Option<bool>,
    pub image_type_limit: Option<u32>,
    pub enable_image_types: Option<&'a str>,
    pub next_up_date_cutoff: Option<&'a str>,
    pub enable_total_record_count: Option<bool>,
    pub enable_images: Option<bool>,
    pub disable_first_episode: Option<bool>,
    pub enable_resumable: Option<bool>,
    pub enable_rewatching: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ImageType {
    Primary,
    Art,
    Backdrop,
    Banner,
    Logo,
    Thumb,
    Disc,
    Box,
    Screenshot,
    Menu,
    Chapter,
    BoxRear,
    Profile,
}
impl ImageType {
    pub fn name(&self) -> &'static str {
        match self {
            ImageType::Primary => "Primary",
            ImageType::Art => "Art",
            ImageType::Backdrop => "Backdrop",
            ImageType::Banner => "Banner",
            ImageType::Logo => "Logo",
            ImageType::Thumb => "Thumb",
            ImageType::Disc => "Disc",
            ImageType::Box => "Box",
            ImageType::Screenshot => "Screenshot",
            ImageType::Menu => "Menu",
            ImageType::Chapter => "Chapter",
            ImageType::BoxRear => "BoxRear",
            ImageType::Profile => "Profile",
        }
    }
}

impl Display for ImageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum MediaType {
    Unknown,
    Video,
    Audio,
    Photo,
    Book,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "Type")]
pub enum ItemType {
    #[serde(rename_all = "PascalCase")]
    Movie ,
    #[serde(rename_all = "PascalCase")]
    Episode {
        season_id: Option<String>,
        season_name: Option<String>,
        series_id: String,
        series_name: String,
    },
    #[serde(rename_all = "PascalCase")]
    Season {
        series_id: String,
        series_name: String,
    },
    Series,
    Playlist,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    pub playback_position_ticks: u64,
    pub unplayed_item_count: Option<u64>,

    pub is_favorite: bool,
    pub played: bool,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SetUserData {
    pub playback_position_ticks: Option<u64>,
    pub unplayed_item_count: Option<u64>,
    pub is_favorite: Option<bool>,
    pub played: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct MediaItem {
    pub id: String,
    pub image_tags: Option<HashMap<ImageType, String>>,
    pub media_type: MediaType,
    pub name: String,
    pub sort_name: Option<String>,
    pub overview: Option<String>,
    #[serde(flatten)]
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub user_data: Option<UserData>,
    #[serde(rename = "IndexNumber")]
    pub episode_index: Option<u64>,
    #[serde(rename = "ParentIndexNumber")]
    pub season_index: Option<u64>,
}

impl<Sha: ShaImpl> JellyfinClient<Auth, Sha> {
    #[instrument(skip(self))]
    pub async fn get_user_items_resume(
        &self,
        query: &GetResumeQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        let req = self
            .get(format!("{}UserItems/Resume", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }

    #[instrument(skip(self))]
    pub async fn get_shows_next_up(
        &self,
        query: &GetNextUpQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        let req = self
            .get(format!("{}Shows/NextUp", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }

    pub async fn get_items(
        &self,
        query: &GetItemsQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        let req = self
            .get(format!("{}Items", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.into())
    }

    pub async fn get_item(
        &self,
        id: &str,
        user_id: Option<&str>,
    ) -> Result<JsonResponse<MediaItem>> {
        let req = self
            .get(format!("{}Items/{id}", self.url))
            .query(&UserIdQuery { user_id })
            .send()
            .await?
            .error_for_status()?;
        Ok(req.into())
    }

    pub async fn set_user_data(&self, item: &str, data: &SetUserData) -> Result<()> {
        let _ = self
            .post(format!("{}UserItems/{item}/UserData", self.url))
            .json(data)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub fn get_video_url(&self, item: &MediaItem) -> String {
        format!("{}Items/{}/Download", self.url, item.id)
    }
}
