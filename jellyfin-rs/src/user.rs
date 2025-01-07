use super::err::Result;
use serde::Deserialize;
use serde::Serialize;

use super::session::SessionInfo;
use crate::sha::Sha256;
use crate::AuthStatus;
use crate::Authed;
use crate::JellyfinClient;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserIdQuery<'id> {
    user_id: &'id str,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    pub name: String,
    pub server_id: String,
    pub server_name: Option<String>,
    pub id: String,
    pub primary_image_tag: Option<String>,
    pub has_password: bool,
    pub has_configured_password: bool,
    pub has_configured_easy_password: bool,
    pub enable_auto_login: bool,
    pub last_login_date: Option<String>,
    pub last_activity_date: Option<String>,
    pub configuration: UserConfiguration,
    pub policy: UserPolicy,
    pub primary_image_aspect_ratio: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStream {
    //         "Codec": "string",
    //         "CodecTag": "string",
    //         "Language": "string",
    //         "ColorRange": "string",
    //         "ColorSpace": "string",
    //         "ColorTransfer": "string",
    //         "ColorPrimaries": "string",
    //         "DvVersionMajor": 0,
    //         "DvVersionMinor": 0,
    //         "DvProfile": 0,
    //         "DvLevel": 0,
    //         "RpuPresentFlag": 0,
    //         "ElPresentFlag": 0,
    //         "BlPresentFlag": 0,
    //         "DvBlSignalCompatibilityId": 0,
    //         "Comment": "string",
    //         "TimeBase": "string",
    //         "CodecTimeBase": "string",
    //         "Title": "string",
    //         "VideoRange": "string",
    //         "VideoRangeType": "string",
    //         "VideoDoViTitle": "string",
    //         "LocalizedUndefined": "string",
    //         "LocalizedDefault": "string",
    //         "LocalizedForced": "string",
    //         "LocalizedExternal": "string",
    //         "DisplayTitle": "string",
    //         "NalLengthSize": "string",
    //         "IsInterlaced": true,
    //         "IsAVC": true,
    //         "ChannelLayout": "string",
    //         "BitRate": 0,
    //         "BitDepth": 0,
    //         "RefFrames": 0,
    //         "PacketLength": 0,
    //         "Channels": 0,
    //         "SampleRate": 0,
    //         "IsDefault": true,
    //         "IsForced": true,
    //         "Height": 0,
    //         "Width": 0,
    //         "AverageFrameRate": 0,
    //         "RealFrameRate": 0,
    //         "Profile": "string",
    //         "Type": "Audio",
    //         "AspectRatio": "string",
    //         "Index": 0,
    //         "Score": 0,
    //         "IsExternal": true,
    //         "DeliveryMethod": "Encode",
    //         "DeliveryUrl": "string",
    //         "IsExternalUrl": true,
    //         "IsTextSubtitleStream": true,
    //         "SupportsExternalStream": true,
    //         "Path": "string",
    //         "PixelFormat": "string",
    //         "Level": 0,
    //         "IsAnamorphic": true
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSource {
    pub protocol: String,
    pub id: String,
    pub path: String,
    pub encoder_path: Option<String>,
    pub encoder_protocol: Option<String>,
    pub r#type: String,
    pub container: String,
    pub size: i64,
    pub name: String,
    pub is_remote: bool,
    pub etag: Option<String>,
    pub run_time_ticks: i64,
    pub read_at_native_framerate: bool,
    pub ignore_dts: bool,
    pub ignore_index: bool,
    pub gen_pts_input: bool,
    pub supports_transcoding: bool,
    pub supports_direct_stream: bool,
    pub supports_direct_play: bool,
    pub is_infinite_stream: bool,
    pub requires_opening: bool,
    pub open_token: Option<String>,
    pub requires_closing: bool,
    pub live_stream_id: Option<String>,
    pub buffer_ms: Option<i64>,
    pub requires_looping: bool,
    pub supports_probing: bool,
    pub video_type: String,
    pub iso_type: Option<String>,
    pub video_3d_format: Option<String>,
    pub media_streams: Vec<MediaStream>,
    // media_attachments: Vec<MediaAttachment>,
    //     "MediaAttachments": [
    //       {
    //         "Codec": "string",
    //         "CodecTag": "string",
    //         "Comment": "string",
    //         "Index": 0,
    //         "FileName": "string",
    //         "MimeType": "string",
    //         "DeliveryUrl": "string"
    //       }
    //     ],
    pub formats: Vec<String>,
    pub bitrate: i64,
    pub timestamp: Option<String>,
    // required_http_headers: serde_json::Map<String, serde_json::Value>,
    pub transcoding_url: Option<String>,
    pub transcoding_sub_protocol: Option<String>,
    pub transcoding_container: Option<String>,
    pub analyze_duration_ms: Option<i64>,
    pub default_audio_stream_index: i64,
    pub default_subtitle_stream_index: i64,
    //     "Formats": [
    //       "string"
    //     ],
    //     "RequiredHttpHeaders": {
    //       "property1": "string",
    //       "property2": "string"
    //     },
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserItem {
    pub name: String,
    pub original_title: Option<String>,
    pub server_id: String,
    pub id: String,
    pub etag: String,
    pub source_type: Option<String>,
    pub playlist_item_id: Option<String>,
    pub date_created: String,
    pub date_last_media_added: Option<String>,
    pub extra_type: Option<String>,
    // airs_before_season_number: i64,
    // airs_after_season_number: i64,
    // airs_before_episode_number: i64,
    pub can_delete: bool,
    pub can_download: bool,
    pub has_subtitles: bool,
    pub preferred_metadata_language: Option<String>,
    pub preferred_metadata_country_code: Option<String>,
    pub supports_sync: Option<bool>,
    pub container: String,
    pub sort_name: String,
    pub forced_sort_name: Option<String>,
    pub video_3d_format: Option<String>,
    pub premiere_date: Option<String>,
    pub media_sources: Vec<MediaSource>,
    // "ExternalUrls": [
    //   {
    //     "Name": "string",
    //     "Url": "string"
    //   }
    // ],
    pub critic_rating: Option<i64>,
    // "ProductionLocations": [
    //   "string"
    // ],
    pub path: String,
    // enable_media_source_display: bool,
    // official_rating: String,
    // custom_rating: String,
    // channel_id: String,
    // channel_name: String,
    // overview: String,
    // taglines: Vec<String>,
    // genres: Vec<String>,
    // community_rating: i64,
    // cumulative_run_time_ticks: i64,
    // run_time_ticks: i64,
    // play_access: String,
    // aspect_ratio: String,
    // production_year: i64,
    // is_place_holder: bool,
    // number: String,
    // channel_number: String,
    // index_number: i64,
    // index_number_end: i64,
    // parent_index_number: i64,
    // "RemoteTrailers": [
    //   {
    //     "Url": "string",
    //     "Name": "string"
    //   }
    // ],
    // "ProviderIds": {
    //   "property1": "string",
    //   "property2": "string"
    // },
    pub is_hd: Option<bool>,
    pub is_folder: bool,
    pub parent_id: String,
    pub r#type: String,
    // "People": [
    //   {
    //     "Name": "string",
    //     "Id": "38a5a5bb-dc30-49a2-b175-1de0d1488c43",
    //     "Role": "string",
    //     "Type": "string",
    //     "PrimaryImageTag": "string",
    //     "ImageBlurHashes": {
    //       "Primary": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Art": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Backdrop": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Banner": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Logo": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Thumb": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Disc": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Box": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Screenshot": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Menu": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Chapter": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "BoxRear": {
    //         "property1": "string",
    //         "property2": "string"
    //       },
    //       "Profile": {
    //         "property1": "string",
    //         "property2": "string"
    //       }
    //     }
    //   }
    // ],
    // "Studios": [
    //   {
    //     "Name": "string",
    //     "Id": "38a5a5bb-dc30-49a2-b175-1de0d1488c43"
    //   }
    // ],
    // "GenreItems": [
    //   {
    //     "Name": "string",
    //     "Id": "38a5a5bb-dc30-49a2-b175-1de0d1488c43"
    //   }
    // ],
    // parent_logo_item_id: String,
    // parent_backdrop_item_id: String,
    // parent_backdrop_image_tags: Vec<String>,
    // local_trailer_count: i64,
    // "UserData": {
    //   "Rating": 0,
    //   "PlayedPercentage": 0,
    //   "UnplayedItemCount": 0,
    //   "PlaybackPositionTicks": 0,
    //   "PlayCount": 0,
    //   "IsFavorite": true,
    //   "Likes": true,
    //   "LastPlayedDate": "2019-08-24T14:15:22Z",
    //   "Played": true,
    //   "Key": "string",
    //   "ItemId": "string"
    // },
    // recursive_item_count: i64,
    // child_count: i64,
    // series_name: String,
    // series_id: String,
    // season_id: String,
    // special_feature_count: i64,
    // display_preferences_id: String,
    // status: String,
    // airtime: String,
    // air_days: Vec<String>,
    // tags: Vec<String>,
    // primary_image_aspect_ratio: String,
    // artists: Vec<String>,
    // artist_items: Vec<String>,
    // album: String,
    // collection_type: String,
    // display_order: String,
    // album_id: String,
    // album_primary_image_tag: String,
    // series_primary_image_tag: String,
    // album_artist: String,
    // "AlbumArtists": [
    //   {
    //     "Name": "string",
    //     "Id": "38a5a5bb-dc30-49a2-b175-1de0d1488c43"
    //   }
    // ],
    // season_name: String,
    // "MediaStreams": [
    //   {
    //     "Codec": "string",
    //     "CodecTag": "string",
    //     "Language": "string",
    //     "ColorRange": "string",
    //     "ColorSpace": "string",
    //     "ColorTransfer": "string",
    //     "ColorPrimaries": "string",
    //     "DvVersionMajor": 0,
    //     "DvVersionMinor": 0,
    //     "DvProfile": 0,
    //     "DvLevel": 0,
    //     "RpuPresentFlag": 0,
    //     "ElPresentFlag": 0,
    //     "BlPresentFlag": 0,
    //     "DvBlSignalCompatibilityId": 0,
    //     "Comment": "string",
    //     "TimeBase": "string",
    //     "CodecTimeBase": "string",
    //     "Title": "string",
    //     "VideoRange": "string",
    //     "VideoRangeType": "string",
    //     "VideoDoViTitle": "string",
    //     "LocalizedUndefined": "string",
    //     "LocalizedDefault": "string",
    //     "LocalizedForced": "string",
    //     "LocalizedExternal": "string",
    //     "DisplayTitle": "string",
    //     "NalLengthSize": "string",
    //     "IsInterlaced": true,
    //     "IsAVC": true,
    //     "ChannelLayout": "string",
    //     "BitRate": 0,
    //     "BitDepth": 0,
    //     "RefFrames": 0,
    //     "PacketLength": 0,
    //     "Channels": 0,
    //     "SampleRate": 0,
    //     "IsDefault": true,
    //     "IsForced": true,
    //     "Height": 0,
    //     "Width": 0,
    //     "AverageFrameRate": 0,
    //     "RealFrameRate": 0,
    //     "Profile": "string",
    //     "Type": "Audio",
    //     "AspectRatio": "string",
    //     "Index": 0,
    //     "Score": 0,
    //     "IsExternal": true,
    //     "DeliveryMethod": "Encode",
    //     "DeliveryUrl": "string",
    //     "IsExternalUrl": true,
    //     "IsTextSubtitleStream": true,
    //     "SupportsExternalStream": true,
    //     "Path": "string",
    //     "PixelFormat": "string",
    //     "Level": 0,
    //     "IsAnamorphic": true
    //   }
    // ],
    // video_type: String,
    // part_count: i64,
    // media_source_count: i64,
    // "ImageTags": {
    //   "property1": "string",
    //   "property2": "string"
    // },
    // backdrop_image_tags: Vec<String>,
    // screenshot_image_tags: Vec<String>,
    // parent_logo_image_tag: String,
    // parent_art_item_id: String,
    // parent_art_image_tag: String,
    // series_thumb_image_tag: String,
    // "ImageBlurHashes": {
    //   "Primary": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Art": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Backdrop": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Banner": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Logo": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Thumb": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Disc": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Box": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Screenshot": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Menu": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Chapter": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "BoxRear": {
    //     "property1": "string",
    //     "property2": "string"
    //   },
    //   "Profile": {
    //     "property1": "string",
    //     "property2": "string"
    //   }
    // },
    // series_studio: String,
    // parent_thumb_item_id: String,
    // parent_thumb_image_tag: String,
    // parent_primary_image_item_id: String,
    // parent_primary_image_tag: String,
    // "Chapters": [
    //   {
    //     "StartPositionTicks": 0,
    //     "Name": "string",
    //     "ImagePath": "string",
    //     "ImageDateModified": "2019-08-24T14:15:22Z",
    //     "ImageTag": "string"
    //   }
    // ],
    // location_type: String,
    // iso_type: String,
    // media_type: String,
    // end_date: String,
    // locked_fields: Vec<String>,
    // trailer_count: i64,
    // movie_count: i64,
    // series_count: i64,
    // program_count: i64,
    // episode_count: i64,
    // song_count: i64,
    // album_count: i64,
    // artist_count: i64,
    // music_video_count: i64,
    // lock_data: bool,
    // width: i64,
    // height: i64,
    // camera_make: String,
    // camera_model: String,
    // software: String,
    // exposure_time: i64,
    // focal_length: i64,
    // image_orientation: String,
    // aperture: i64,
    // shutter_speed: i64,
    // latitude: i64,
    // longitude: i64,
    // altitude: i64,
    // iso_speed_rating: i64,
    // series_timer_id: String,
    // program_id: String,
    // channel_primary_image_tag: String,
    // start_date: String,
    // completion_percentage: i64,
    // is_repeat: bool,
    // episode_title: String,
    // channel_type: String,
    // audio: String,
    // is_movie: bool,
    // is_sports: bool,
    // is_series: bool,
    // is_live: bool,
    // is_news: bool,
    // is_kids: bool,
    // is_premiere: bool,
    // timer_id: String,
    // "CurrentProgram": {}
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserConfiguration {
    pub audio_language_preference: Option<String>,
    pub play_default_audio_track: bool,
    pub subtitle_language_preference: String,
    pub display_missing_episodes: bool,
    pub grouped_folders: Vec<String>,
    pub subtitle_mode: String,
    pub display_collections_view: bool,
    pub enable_local_password: bool,
    pub ordered_views: Vec<String>,
    pub latest_items_excludes: Vec<String>,
    pub my_media_excludes: Vec<String>,
    pub hide_played_in_latest: bool,
    pub remember_audio_selections: bool,
    pub remember_subtitle_selections: bool,
    pub enable_next_episode_auto_play: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserPolicy {
    pub is_administrator: bool,
    pub is_hidden: bool,
    pub is_disabled: bool,
    pub max_parental_rating: Option<i64>,
    pub blocked_tags: Vec<String>,
    pub enable_user_preference_access: bool,
    pub access_schedules: Vec<UserAccessSchedule>,
    pub block_unrated_items: Vec<String>,
    pub enable_remote_control_of_other_users: bool,
    pub enable_shared_device_control: bool,
    pub enable_remote_access: bool,
    pub enable_live_tv_management: bool,
    pub enable_live_tv_access: bool,
    pub enable_media_playback: bool,
    pub enable_audio_playback_transcoding: bool,
    pub enable_video_playback_transcoding: bool,
    pub enable_playback_remuxing: bool,
    pub force_remote_source_transcoding: bool,
    pub enable_content_deletion: bool,
    pub enable_content_deletion_from_folders: Vec<String>,
    pub enable_content_downloading: bool,
    pub enable_sync_transcoding: bool,
    pub enable_media_conversion: bool,
    pub enabled_devices: Vec<String>,
    pub enable_all_devices: bool,
    pub enabled_channels: Vec<String>,
    pub enable_all_channels: bool,
    pub enabled_folders: Vec<String>,
    pub enable_all_folders: bool,
    pub invalid_login_attempt_count: i64,
    pub login_attempts_before_lockout: i64,
    pub max_active_sessions: i64,
    pub enable_public_sharing: bool,
    pub blocked_media_folders: Vec<String>,
    pub blocked_channels: Vec<String>,
    pub remote_client_bitrate_limit: i64,
    pub authentication_provider_id: String,
    pub password_reset_provider_id: String,
    pub sync_play_access: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserAccessSchedule {
    pub user_id: String,
    pub day_of_week: String,
    pub start_hour: i64,
    pub end_hour: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserAuth {
    pub user: User,
    pub session_info: SessionInfo,
    pub access_token: String,
    pub server_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GetUsersQuery {
    is_hidden: bool,
    is_disabled: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuthUserStdQuery {
    pw: String,
    password: String,
}

impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    /// Gets a list of all users that the `UserAuth` has access to, given some filters.
    pub async fn get_users(&self, is_hidden: bool, is_disabled: bool) -> Result<Vec<User>> {
        let req = self
            .get(format!("{}Users", self.url,))
            .query(&GetUsersQuery {
                is_hidden,
                is_disabled,
            })
            .send()
            .await?;
        Ok(req.json().await?)
    }
    pub async fn get_user_by_id(&self, id: impl AsRef<str>) -> Result<User> {
        let req = self
            .get(format!("{}Users/{}", self.url, id.as_ref()))
            .send()
            .await?;
        Ok(req.json().await?)
    }
    pub async fn delete_user(&self, id: impl AsRef<str>) -> Result<()> {
        let _req = self
            .delete(format!("{}Users/{}", self.url, id.as_ref()))
            .send()
            .await?;
        Ok(())
    }
    pub async fn update_user(&self, id: impl AsRef<str>, new_info: &User) -> Result<()> {
        let _req = self
            .post(format!("{}Users", self.url))
            .query(&UserIdQuery {
                user_id: id.as_ref(),
            })
            .json(new_info)
            .send()
            .await?;
        Ok(())
    }
    pub async fn update_user_conf(
        &self,
        id: impl AsRef<str>,
        new_conf: &UserConfiguration,
    ) -> Result<()> {
        let _req = self
            .post(format!("{}Users/Configuration", self.url))
            .query(&UserIdQuery {
                user_id: id.as_ref(),
            })
            .json(new_conf)
            .send()
            .await?;
        Ok(())
    }
    pub async fn update_user_password(
        &self,
        id: impl AsRef<str>,
        new_password: impl AsRef<str>,
    ) -> Result<()> {
        let _req = self
            .post(format!("{}Users/Password", self.url))
            .query(&UserIdQuery {
                user_id: id.as_ref(),
            })
            .json(&NewPwReq {
                new_pw: new_password.as_ref(),
            })
            .send()
            .await?;
        Ok(())
    }
    pub async fn update_user_policy(
        &self,
        id: impl AsRef<str>,
        new_policy: &UserPolicy,
    ) -> Result<()> {
        let req = self
            .post(format!("{}Users/{}/Policy", self.url, id.as_ref()))
            .json(&new_policy)
            .send()
            .await?;
        let _req = req.error_for_status()?;
        Ok(())
    }
    pub async fn get_user_by_auth(&self) -> Result<User> {
        let req = self.get(format!("{}Users/Me", self.url)).send().await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
    pub async fn create_user(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<User> {
        let req = self
            .post(format!("{}Users/New", self.url))
            .json(&CreateUserReq {
                name: username.as_ref(),
                password: password.as_ref(),
            })
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct NewPwReq<'pw> {
    new_pw: &'pw str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct ForgotPwReq<'s> {
    entered_username: &'s str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForgotPasswordAction {
    ContactAdmin,
    PinCode,
    InNetworkRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ForgotPasswordResponse {
    action: ForgotPasswordAction,
    pin_file: Option<String>,
    pin_expiration_date: Option<String>,
}

impl<Auth: AuthStatus, Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub async fn user_forgot_password(
        &self,
        username: impl AsRef<str>,
    ) -> Result<ForgotPasswordResponse> {
        let req = self
            .client
            .post(format!("{}Users/ForgotPassword", self.url))
            .json(&ForgotPwReq {
                entered_username: username.as_ref(),
            })
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
    pub async fn user_redeem_forgot_password_pin(
        &self,
        pin: impl AsRef<str>,
    ) -> Result<RedeemForgotPasswordResponse> {
        let req = self
            .client
            .post(format!("{}Users/ForgotPassword/Pin", self.url))
            .json(&RedeemForgotPasswordReq { pin: pin.as_ref() })
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
    pub async fn get_public_user_list(&self) -> Result<Vec<User>> {
        let req = self
            .client
            .get(format!("{}Users/Public", self.url))
            .send()
            .await?;
        let req = req.error_for_status()?;
        Ok(req.json().await?)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct RedeemForgotPasswordReq<'s> {
    pin: &'s str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RedeemForgotPasswordResponse {
    success: bool,
    users_reset: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateUserReq<'s> {
    name: &'s str,
    password: &'s str,
}
