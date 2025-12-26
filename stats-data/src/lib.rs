use std::sync::{Arc, atomic::AtomicU64};

#[derive(Default)]
pub struct StatsData {
    pub image_fetches: AtomicU64,
    pub db_image_cache_hits: AtomicU64,
    pub memory_image_cache_hits: AtomicU64,
}

pub type Stats = Arc<StatsData>;
