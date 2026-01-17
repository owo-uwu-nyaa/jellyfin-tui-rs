use std::collections::{HashMap, HashSet};

use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};
use entries::image::{JellyfinImage, available::ImagesAvailable, cache::ImageProtocolCache};
use futures_util::future::{try_join, try_join_all};
use jellyfin::{
    JellyfinClient,
    image::select_images_owned,
    items::ImageType,
    library::Library,
    scheduled_tasks::{ScheduledTask, known_keys},
};
use jellyhaj_core::context::{DB, ImagePicker, Stats};

pub struct LibraryWidget {
    libraries: Vec<(Library, Option<JellyfinImage>)>,
    refresh_task: ScheduledTask,
}

impl LibraryWidget {
    pub async fn new(
        jellyfin: &JellyfinClient,
        db: &DB,
        available: &ImagesAvailable,
        cache: &ImageProtocolCache,
        picker: &ImagePicker,
        stats: &Stats,
    ) -> Result<Self> {
        let libraries = async {
            try_join_all(
                jellyfin
                    .get_libraries()
                    .await
                    .context("getting jellyfin libraries")?
                    .deserialize()
                    .await
                    .context("deserializing jellyfin libraries")?
                    .into_iter()
                    .map(|item| async move {
                        get_image(&item.item_id, jellyfin).await.map(|image| {
                            let image = image.map(|image| {
                                JellyfinImage::new(
                                    item.item_id.clone(),
                                    image.tag,
                                    image.image_type,
                                    jellyfin.clone(),
                                    db.clone(),
                                    available.clone(),
                                    cache.clone(),
                                    picker.clone(),
                                    stats.clone(),
                                )
                            });
                            (item, image)
                        })
                    }),
            )
            .await
        };
        let refresh_task = async {
            jellyfin
                .get_scheduled_tasks()
                .await
                .context("getting scheduled tasks")?
                .deserialize()
                .await
                .context("deserializing scheduled tasks")?
                .into_iter()
                .find(|task| task.key == known_keys::REFRESH_LIBRARY)
                .ok_or_eyre("no refresh library task")
        };
        let (libraries, refresh_task) = try_join(libraries, refresh_task).await?;
        Ok(Self {
            libraries,
            refresh_task,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_update(
    old: Vec<(Library, Option<JellyfinImage>)>,
    new: Vec<LibraryUpdate>,
    jellyfin: &JellyfinClient,
    db: &DB,
    available: &ImagesAvailable,
    cache: &ImageProtocolCache,
    picker: &ImagePicker,
    stats: &Stats,
) -> Vec<(Library, Option<JellyfinImage>)> {
    let mut old: HashMap<_, _> = old
        .into_iter()
        .filter_map(|(l, i)| i.map(|i| (l.item_id, i)))
        .collect();
    new.into_iter()
        .map(|library| match library {
            LibraryUpdate::New {
                val,
                image: Some(image),
            } => {
                let image = JellyfinImage::new(
                    val.item_id.clone(),
                    image.tag,
                    image.image_type,
                    jellyfin.clone(),
                    db.clone(),
                    available.clone(),
                    cache.clone(),
                    picker.clone(),
                    stats.clone(),
                );
                (val, Some(image))
            }
            LibraryUpdate::New { val, image: None } => (val, None),
            LibraryUpdate::Existing { val } => {
                let i = old.remove(&val.item_id);
                (val, i)
            }
        })
        .collect()
}

pub async fn get_update(
    existing: HashSet<String>,
    task_id: &str,
    jellyfin: &JellyfinClient,
) -> Result<(Vec<LibraryUpdate>, ScheduledTask)> {
    async fn update_libraries(
        existing: HashSet<String>,
        jellyfin: &JellyfinClient,
    ) -> Result<Vec<LibraryUpdate>> {
        try_join_all(
            jellyfin
                .get_libraries()
                .await
                .context("getting libraries")?
                .deserialize()
                .await
                .context("deserializing libraries")?
                .into_iter()
                .map(|l| {
                    let new = !existing.contains(&l.item_id);
                    async move {
                        if new {
                            match get_image(&l.item_id, jellyfin).await {
                                Err(e) => Err(e),
                                Ok(v) => Ok(LibraryUpdate::New { val: l, image: v }),
                            }
                        } else {
                            Ok(LibraryUpdate::Existing { val: l })
                        }
                    }
                }),
        )
        .await
    }

    async fn get_task(task_id: &str, jellyfin: &JellyfinClient) -> Result<ScheduledTask> {
        jellyfin
            .get_scheduled_task(task_id)
            .await
            .context("getting refresh task")?
            .deserialize()
            .await
            .context("deserializing refresh task")
    }
    tokio::try_join!(
        update_libraries(existing, jellyfin),
        get_task(task_id, jellyfin)
    )
}

pub struct ImageID {
    image_type: ImageType,
    tag: String,
}

async fn get_image(id: &str, jellyfin: &JellyfinClient) -> Result<Option<ImageID>> {
    let item = jellyfin.get_item(id, None).await?.deserialize().await?;
    Ok(
        if let Some((image_type, tag)) = select_images_owned(item).next() {
            Some(ImageID { image_type, tag })
        } else {
            None
        },
    )
}

pub enum LibraryUpdate {
    New {
        val: Library,
        image: Option<ImageID>,
    },
    Existing {
        val: Library,
    },
}
