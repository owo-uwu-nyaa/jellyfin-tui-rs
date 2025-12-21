use jellyfin::{
    JellyfinClient,
    image::{GetImageQuery, select_images},
};
use player_core::{PlaylistItem, PlaylistItemId};
use serde::{Deserialize, Serialize, Serializer};
use std::{collections::HashMap, result::Result as StdResult};
use tracing::error;
use zbus::{
    fdo::{Error, Result},
    zvariant::{ObjectPath, OwnedObjectPath, Type, Value, as_value},
};

pub fn track_id_as_object(id: Option<PlaylistItemId>) -> OwnedObjectPath {
    if let Some(id) = id {
        let id = id.id;
        OwnedObjectPath::try_from(format!("/trackids/{id}"))
    } else {
        OwnedObjectPath::try_from("/org/mpris/MediaPlayer2/TrackList/NoTrack")
    }
    .expect("should always be valid")
}

fn serialize_track_id<S: Serializer>(
    id: &Option<PlaylistItemId>,
    s: S,
) -> StdResult<S::Ok, S::Error> {
    let owned = track_id_as_object(*id);
    as_value::serialize(&owned, s)
}

pub fn parse_track_id(object: &ObjectPath<'_>) -> Result<Option<PlaylistItemId>> {
    let object = object.as_str();
    if object == "/org/mpris/MediaPlayer2/TrackList/NoTrack" {
        Ok(None)
    } else {
        let start = "/trackids/";
        if start
            == object.get(0..start.len()).ok_or_else(|| {
                Error::InvalidArgs("track id object path has wrong base".to_owned())
            })?
        {
            let id = &object[start.len()..];
            if id.is_empty() {
                Err(Error::InvalidArgs("track id is empty".to_owned()))
            } else if id.as_bytes().iter().all(|c| (0x30..=0x39).contains(c)) {
                Ok(Some(PlaylistItemId {
                    id: id
                        .parse()
                        .map_err(|_| Error::InvalidArgs("integer overflow in id".to_owned()))?,
                }))
            } else {
                Err(Error::InvalidArgs("track id is no number".to_owned()))
            }
        } else {
            Err(Error::InvalidArgs(
                "track id object path has wrong base".to_owned(),
            ))
        }
    }
}

#[derive(Deserialize, Serialize, Type, Value, PartialEq, Debug)]
#[zvariant(signature = "s")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

#[derive(Deserialize, Serialize, Type, Value, PartialEq, Debug)]
#[zvariant(signature = "s")]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}

#[derive(Debug, Default, Serialize, Type)]
#[zvariant(signature = "a{sv}")]
pub struct Metadata {
    #[serde(rename = "mpris:trackid", serialize_with = "serialize_track_id")]
    trackid: Option<PlaylistItemId>,
    #[serde(
        rename = "mpris:length",
        skip_serializing_if = "Option::is_none",
        with = "as_value::optional"
    )]
    length: Option<f64>,
    #[serde(
        rename = "mpris:artUrl",
        skip_serializing_if = "Option::is_none",
        with = "as_value::optional"
    )]
    image: Option<String>,
    #[serde(
        rename = "xesam:title",
        skip_serializing_if = "Option::is_none",
        with = "as_value::optional"
    )]
    title: Option<String>,
}

impl From<Metadata> for Value<'static> {
    fn from(s: Metadata) -> Self {
        let mut fields = HashMap::new();
        fields.insert("mpris:trackid", Value::from(track_id_as_object(s.trackid)));
        if let Some(v) = s.length {
            fields.insert("mpris:length", Value::from(v));
        }
        if let Some(v) = s.image {
            fields.insert("mpris:artUrl", Value::from(v));
        }
        if let Some(v) = s.title {
            fields.insert("xesam:title", Value::from(v));
        }
        fields.into()
    }
}

impl Metadata {
    pub fn new(item: &PlaylistItem, jellyfin: &JellyfinClient) -> Self {
        let length = item.item.run_time_ticks.map(|v| (v as f64) / 10000000.0);
        let image = select_images(&item.item)
            .next()
            .and_then(|(image_type, tag)| {
                jellyfin
                    .get_image_uri(
                        &item.item.id,
                        image_type,
                        &GetImageQuery {
                            tag: Some(tag),
                            format: Some("Webp"),
                            max_width: None,
                            max_height: None,
                        },
                    )
                    .inspect_err(|e| error!("error constructing image uri: {e:?}"))
                    .ok()
                    .map(|uri| uri.to_string())
            });
        Self {
            trackid: Some(item.id),
            length,
            image,
            title: Some(item.item.name.clone()),
        }
    }
}
