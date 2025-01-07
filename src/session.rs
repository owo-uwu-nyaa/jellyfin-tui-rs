use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::items::MediaItem;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionInfo {
    pub play_state: PlayState,
    pub additional_users: Vec<AdditionalUser>,
    pub capabilities: Capabilities,
    pub remote_end_point: String,
    pub playable_media_types: Vec<String>,
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub client: String,
    pub last_activity_date: Option<String>,
    pub last_playback_check_in: String,
    pub device_name: String,
    pub device_type: Option<String>,
    pub now_playing_item: Option<MediaItem>,
    pub now_viewing_item: Option<MediaItem>,
    pub device_id: String,
    pub application_version: String,
    pub transcoding_info: Option<TranscodingInfo>,
    pub is_active: bool,
    pub supports_media_control: bool,
    pub supports_remote_control: bool,
    pub now_playing_queue_full_items: Vec<MediaItem>,
    pub has_custom_device_name: bool,
    pub playlist_item_id: Option<String>,
    pub server_id: String,
    pub user_primary_image_tag: Option<String>,
    pub supported_commands: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayState {
    pub position_ticks: Option<i64>,
    pub can_seek: bool,
    pub is_paused: bool,
    pub is_muted: bool,
    pub volume_level: Option<i64>,
    pub audio_stream_index: Option<i64>,
    pub subtitle_stream_index: Option<i64>,
    pub media_source_id: Option<String>,
    pub play_method: Option<String>,
    pub repeat_mode: String,
    pub live_stream_id: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AdditionalUser {
    pub user_id: String,
    pub user_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Capabilities {
    pub playable_media_types: Vec<String>,
    pub supported_commands: Vec<String>,
    pub supports_media_control: bool,
    pub supports_content_uploading: Option<bool>,
    pub message_callback_url: Option<String>,
    pub supports_persistent_identifier: bool,
    pub supports_sync: Option<bool>,
    pub device_profile: Option<DeviceProfile>,
    pub app_store_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceProfile {
    pub name: String,
    pub id: String,
    pub identification: Identification,
    pub friendly_name: String,
    pub manufacturer: String,
    pub manufacturer_url: String,
    pub model_name: String,
    pub model_description: String,
    pub model_number: String,
    pub model_url: String,
    pub serial_number: String,
    pub enable_album_art_in_didl: bool,
    pub enable_single_album_art_limit: bool,
    pub enable_single_subtitle_limit: bool,
    pub supported_media_types: String,
    pub user_id: String,
    pub album_art_pn: String,
    pub max_album_art_width: i64,
    pub max_album_art_height: i64,
    pub max_icon_width: i64,
    pub max_icon_height: i64,
    pub max_streaming_bitrate: i64,
    pub max_static_bitrate: i64,
    pub music_streaming_transcoding_bitrate: i64,
    pub max_static_music_bitrate: i64,
    pub sony_aggregation_flags: String,
    pub protocol_info: String,
    pub timeline_offset_seconds: i64,
    pub requires_plain_video_items: bool,
    pub requires_plain_folders: bool,
    pub enable_msmedia_receiver_registrar: bool,
    pub ignore_transcode_byte_range_requests: bool,
    pub xml_root_attributes: Vec<XmlRootAttribute>,
    pub direct_play_profiles: Vec<DirectPlayProfile>,
    pub transcoding_profiles: Vec<TranscodingProfile>,
    pub container_profiles: Vec<ContainerProfile>,
    pub codec_profiles: Vec<CodecProfile>,
    pub response_profiles: Vec<ResponseProfile>,
    pub subtitle_profiles: Vec<SubtitleProfile>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Identification {
    pub friendly_name: String,
    pub model_number: String,
    pub serial_number: String,
    pub model_name: String,
    pub model_description: String,
    pub model_url: String,
    pub manufacturer: String,
    pub manufacturer_url: String,
    pub headers: Vec<Header>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Header {
    pub name: String,
    pub value: String,
    pub match_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlRootAttribute {
    pub name: String,
    pub value: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DirectPlayProfile {
    pub container: String,
    pub audio_codec: String,
    pub video_codec: String,
    pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TranscodingProfile {
    pub container: String,
    pub type_field: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub protocol: String,
    pub estimate_content_length: bool,
    pub enable_mpegts_m2ts_mode: bool,
    pub transcode_seek_info: String,
    pub copy_timestamps: bool,
    pub context: String,
    pub enable_subtitles_in_manifest: bool,
    pub max_audio_channels: String,
    pub min_segments: i64,
    pub segment_length: i64,
    pub break_on_non_key_frames: bool,
    pub conditions: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ContainerProfile {
    pub type_field: String,
    pub conditions: Vec<Value>,
    pub container: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CodecProfile {
    pub type_field: String,
    pub conditions: Vec<Value>,
    pub apply_conditions: Vec<Value>,
    pub codec: String,
    pub container: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResponseProfile {
    pub container: String,
    pub audio_codec: String,
    pub video_codec: String,
    pub type_field: String,
    pub org_pn: String,
    pub mime_type: String,
    pub conditions: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SubtitleProfile {
    pub format: String,
    pub method: String,
    pub didl_mode: String,
    pub language: String,
    pub container: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TranscodingInfo {
    pub audio_codec: String,
    pub video_codec: String,
    pub container: String,
    pub is_video_direct: bool,
    pub is_audio_direct: bool,
    pub bitrate: i64,
    pub framerate: i64,
    pub completion_percentage: i64,
    pub width: i64,
    pub height: i64,
    pub audio_channels: i64,
    pub hardware_acceleration_type: String,
    pub transcode_reasons: Vec<String>,
}
