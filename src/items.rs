use std::collections::HashMap;
use std::fmt::Display;

use crate::{sha::Sha256, Auth, JellyfinClient, Result};
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

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

impl Display for ImageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
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
        };
        f.write_str(s)
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
    Movie {
        container: String,
    },
    #[serde(rename_all = "PascalCase")]
    Episode {
        container: String,
        season_id: String,
        season_name: String,
        series_id: String,
        series_name: String,
    },
    #[serde(rename_all = "PascalCase")]
    Season {
        series_id: String,
        series_name: String,
    },
    Series,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct MediaItem {
    pub id: String,
    pub image_tags: Option<HashMap<ImageType, String>>,
    pub media_type: MediaType,
    pub name: String,
    #[serde(flatten)]
    #[serde(rename = "type")]
    pub item_type: ItemType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct MediaItemList {
    pub items: Vec<MediaItem>,
    pub start_index: u32,
    pub total_record_count: Option<u32>,
}

impl<Sha: Sha256> JellyfinClient<Auth, Sha> {
    #[instrument(skip(self))]
    pub async fn get_user_items_resume(&self, query: &GetResumeQuery<'_>) -> Result<MediaItemList> {
        let req = self
            .get(format!("{}UserItems/Resume", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }

    #[instrument(skip(self))]
    pub async fn get_shows_next_up(&self, query: &GetNextUpQuery<'_>) -> Result<MediaItemList> {
        let req = self
            .get(format!("{}Shows/NextUp", self.url))
            .query(query)
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
}
