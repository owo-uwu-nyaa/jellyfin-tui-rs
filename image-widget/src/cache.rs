use std::{borrow::Borrow, collections::HashMap, fmt::Debug, hash::Hash, sync::Arc};

use jellyfin::items::ImageType;
use parking_lot::Mutex;
use ratatui::layout::Rect;
use ratatui_image::protocol::Protocol;
use tracing::{instrument, trace};

use crate::image::ImageSize;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ImageProtocolKey {
    pub image_type: ImageType,
    pub item_id: String,
    pub tag: String,
    pub size: ImageSize,
}

impl ImageProtocolKey {
    pub fn new(image_type: ImageType, item_id: String, tag: String, size: ImageSize) -> Self {
        Self {
            image_type,
            item_id,
            tag,
            size,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct ImageProtocolKeyRef<'s> {
    pub image_type: ImageType,
    pub item_id: &'s str,
    pub tag: &'s str,
    pub size: ImageSize,
}

impl<'s> ImageProtocolKeyRef<'s> {
    pub fn new(image_type: ImageType, item_id: &'s str, tag: &'s str, size: ImageSize) -> Self {
        Self {
            image_type,
            item_id,
            tag,
            size,
        }
    }
}

pub trait AsKeyRef {
    fn as_key_ref(&self) -> ImageProtocolKeyRef<'_>;
}
impl AsKeyRef for ImageProtocolKey {
    fn as_key_ref(&self) -> ImageProtocolKeyRef<'_> {
        ImageProtocolKeyRef {
            image_type: self.image_type,
            item_id: &self.item_id,
            tag: &self.tag,
            size: self.size,
        }
    }
}
impl<'s> AsKeyRef for ImageProtocolKeyRef<'s> {
    fn as_key_ref(&self) -> ImageProtocolKeyRef<'_> {
        *self
    }
}

impl<'s> PartialEq for dyn AsKeyRef + 's {
    fn eq(&self, other: &Self) -> bool {
        self.as_key_ref() == other.as_key_ref()
    }
}
impl<'s> Eq for dyn AsKeyRef + 's {}
impl<'s> Hash for dyn AsKeyRef + 's {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_key_ref().hash(state);
    }
}
impl<'s> Debug for dyn AsKeyRef + 's {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.as_key_ref(), f)
    }
}
impl<'s> Borrow<dyn AsKeyRef + 's> for ImageProtocolKey {
    fn borrow(&self) -> &(dyn AsKeyRef + 's) {
        self
    }
}

#[derive(Clone)]
pub struct ImageProtocolCache {
    protocols: Arc<Mutex<HashMap<ImageProtocolKey, (Protocol, Rect)>>>,
}

impl ImageProtocolCache {
    #[instrument(level = "trace", skip(self))]
    pub fn remove(&self, key: &dyn AsKeyRef) -> Option<(Protocol, Rect)> {
        trace!("storing image protocol in cache");
        self.protocols.lock().remove(key)
    }
    #[instrument(level = "trace", skip(self, protocol))]
    pub fn store(&self, protocol: Protocol, final_size: Rect, key: ImageProtocolKey) {
        trace!("storing image protocol in cache");
        self.protocols.lock().insert(key, (protocol, final_size));
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
