use std::collections::HashMap;
use std::fmt::Display;

use crate::Authed;
use crate::request::{NoQuery, RequestBuilderExt};
use crate::{JellyfinClient, JellyfinVec, Result, connect::JsonResponse};
use color_eyre::eyre::Context;
use http::Uri;
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdQuery<'a> {
    pub user_id: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RefreshItemQuery {
    pub recursive: bool,
    pub metadata_refresh_mode: RefreshMode,
    pub image_refresh_mode: RefreshMode,
    pub replace_all_metadata: bool,
    pub replace_all_images: bool,
    pub regenerate_trickplay: bool,
}

impl Default for RefreshItemQuery {
    fn default() -> Self {
        Self {
            recursive: true,
            metadata_refresh_mode: RefreshMode::Default,
            image_refresh_mode: RefreshMode::Default,
            replace_all_metadata: false,
            replace_all_images: false,
            regenerate_trickplay: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum RefreshMode {
    None,
    ValidationOnly,
    Default,
    FullRefresh,
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
    Movie,
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
    pub run_time_ticks: Option<u64>,
}

impl<Auth: Authed> JellyfinClient<Auth> {
    #[instrument(skip(self))]
    pub async fn get_user_items_resume(
        &self,
        query: &GetResumeQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(self.get("/UserItems/Resume", query)?.empty_body()?)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_shows_next_up(
        &self,
        query: &GetNextUpQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(self.get("/Shows/NextUp", query)?.empty_body()?)
            .await
    }

    pub async fn get_items(
        &self,
        query: &GetItemsQuery<'_>,
    ) -> Result<JsonResponse<JellyfinVec<MediaItem>>> {
        self.send_request_json(self.get("/Items", query)?.empty_body()?)
            .await
    }

    pub async fn refresh_item(&self, item: &str, query: &RefreshItemQuery) -> Result<()> {
        self.send_request(
            self.post(
                |base: &mut String| {
                    base.push_str("/Items/");
                    base.push_str(item);
                    base.push_str("/Refresh")
                },
                query,
            )?
            .empty_body()?,
        )
        .await?;
        Ok(())
    }

    pub async fn get_item(
        &self,
        id: &str,
        user_id: Option<&str>,
    ) -> Result<JsonResponse<MediaItem>> {
        self.send_request_json(
            self.get(
                |prefix: &mut String| {
                    prefix.push_str("/Items/");
                    prefix.push_str(id);
                },
                &UserIdQuery { user_id },
            )?
            .empty_body()?,
        )
        .await
    }

    pub async fn set_user_data(&self, item: &str, data: &SetUserData) -> Result<()> {
        self.send_request(
            self.post(
                |prefix: &mut String| {
                    prefix.push_str("/Items/");
                    prefix.push_str(item);
                    prefix.push_str("/UserData");
                },
                NoQuery,
            )?
            .json_body(data)?,
        )
        .await?;
        Ok(())
    }

    pub fn get_video_uri(&self, item: &MediaItem) -> Result<Uri> {
        Uri::builder()
            .scheme(if self.tls() { "https" } else { "http" })
            .authority(self.authority().to_owned())
            .path_and_query(self.build_uri(
                |prefix: &mut String| {
                    prefix.push_str("/Items/");
                    prefix.push_str(&item.id);
                    prefix.push_str("/Download");
                },
                NoQuery,
            )?)
            .build()
            .context("assembling video uri")
    }
}
