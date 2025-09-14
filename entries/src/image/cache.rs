use std::{collections::HashMap, sync::Arc};

use either::Either;
use image::DynamicImage;
use jellyfin::items::ImageType;
use parking_lot::Mutex;
use ratatui_image::protocol::StatefulProtocol;
use tracing::{instrument, trace};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ImageProtocolKey {
    pub image_type: ImageType,
    pub item_id: String,
    pub tag: String,
}

impl ImageProtocolKey {
    pub fn new(image_type: ImageType, item_id: String, tag: String) -> Self {
        Self {
            image_type,
            item_id,
            tag,
        }
    }
}

pub type CachedImage = Either<(StatefulProtocol, u16), DynamicImage>;

#[derive(Clone)]
pub struct ImageProtocolCache {
    protocols: Arc<Mutex<HashMap<ImageProtocolKey, CachedImage>>>,
}

impl ImageProtocolCache {
    #[instrument(level = "trace", skip(self))]
    pub fn remove(&self, key: &ImageProtocolKey) -> Option<CachedImage> {
        trace!("storing image protocol in cache");
        self.protocols.lock().remove(key)
    }
    #[instrument(level = "trace", skip(self, protocol))]
    pub fn store_protocol(&self, protocol: StatefulProtocol, key: ImageProtocolKey, width: u16) {
        trace!("storing image protocol in cache");
        self.protocols
            .lock()
            .insert(key, Either::Left((protocol, width)));
    }
    #[instrument(level = "trace", skip(self, image))]
    pub fn store_image(&self, image: DynamicImage, key: ImageProtocolKey) {
        let mut map = self.protocols.lock();
        if let std::collections::hash_map::Entry::Vacant(entry) = map.entry(key) {
            trace!("storing image in cache");
            entry.insert(Either::Right(image));
        }
    }
    pub fn new() -> Self {
        Self {
            protocols: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for ImageProtocolCache {
    fn default() -> Self {
        Self::new()
    }
}
