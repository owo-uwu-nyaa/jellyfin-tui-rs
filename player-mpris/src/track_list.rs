use std::collections::HashMap;

use jellyfin::JellyfinClient;
use player_core::{Command, PlayerHandle, PlaylistItem, state::SharedPlayerState};
use zbus::{
    fdo::{Error, Result},
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::types::{Metadata, parse_track_id, track_id_as_object};

pub struct TrackList {
    player: PlayerHandle,
    jellyfin: JellyfinClient,
    state: SharedPlayerState,
}

impl TrackList {
    pub fn new(player: PlayerHandle, jellyfin: JellyfinClient, state: SharedPlayerState) -> Self {
        Self {
            player,
            jellyfin,
            state,
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.TrackList", spawn = false)]
impl TrackList {
    fn get_track_metadata(&self, ids: Vec<ObjectPath<'_>>) -> Result<Vec<Metadata>> {
        let state = self.state.lock();
        let indexes: HashMap<_, _> = state
            .playlist
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id, index))
            .collect();
        let mut res = Vec::with_capacity(ids.len());
        for id in ids {
            let id = parse_track_id(&id)?
                .ok_or_else(|| Error::InvalidArgs("NoTrack can have no Metadata".to_string()))?;
            let index = *indexes.get(&id).ok_or_else(|| {
                Error::InvalidArgs(format!("{} is currently not in the track list", id.id))
            })?;
            let item: &PlaylistItem = &state.playlist[index];
            res.push(Metadata::new(item, &self.jellyfin));
        }
        Ok(res)
    }
    fn add_track(&self, _uri: &str, _after: ObjectPath<'_>, _c: bool) -> Result<()> {
        Err(Error::NotSupported(
            "Adding tracks is not supported".to_string(),
        ))
    }
    fn remove_track(&self, track: ObjectPath<'_>) -> Result<()> {
        let id = parse_track_id(&track)?
            .ok_or_else(|| Error::InvalidArgs("NoTrack can not be removed".to_string()))?;
        self.player.send(Command::Remove(id));
        Ok(())
    }
    fn go_to(&self, track: ObjectPath<'_>) -> Result<()> {
        let id = parse_track_id(&track)?
            .ok_or_else(|| Error::InvalidArgs("NoTrack can not be played".to_string()))?;
        self.player.send(Command::Play(id));
        Ok(())
    }

    #[zbus(signal)]
    pub async fn track_list_replaced(
        emitter: &SignalEmitter<'_>,
        tracks: Vec<OwnedObjectPath>,
        current_track: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn track_added(
        emitter: &SignalEmitter<'_>,
        metadata: Metadata,
        after_track: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn track_removed(
        emitter: &SignalEmitter<'_>,
        track_id: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(property(emits_changed_signal = "invalidates"))]
    fn tracks(&self) -> Vec<OwnedObjectPath> {
        self.state
            .lock()
            .playlist
            .iter()
            .map(|i| track_id_as_object(Some(i.id)))
            .collect()
    }
}
