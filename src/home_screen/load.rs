use std::{collections::HashMap, pin::Pin};

use color_eyre::{Result, eyre::Context};
use futures_util::{StreamExt, TryStreamExt, stream};
use jellyfin::{
    Auth, JellyfinClient,
    items::{GetNextUpQuery, GetResumeQuery, MediaItem},
    sha::ShaImpl,
    user_library::GetLatestQuery,
    user_views::{GetUserViewsQuery, UserView, UserViewType},
};
use tracing::{debug, instrument, trace};

use crate::{
    TuiContext,
    fetch::fetch_screen,
    state::{Navigation, NextScreen},
};

#[derive(Debug)]
pub struct HomeScreenData {
    pub resume: Vec<MediaItem>,
    pub next_up: Vec<MediaItem>,
    pub views: Vec<UserView>,
    pub latest: HashMap<String, Vec<MediaItem>>,
}

#[instrument(skip_all)]
pub async fn load_data(
    client: &JellyfinClient<Auth, impl ShaImpl>,
    user_id: &str,
) -> Result<HomeScreenData> {
    debug!("collecting main screen information");
    let user_views = client
        .get_user_views(&GetUserViewsQuery {
            user_id: Some(user_id),
            include_external_content: Some(false),
            include_hidden: Some(false),
            ..Default::default()
        })
        .await
        .context("fetching user views")?
        .deserialize()
        .await
        .context("deserializing user views")?;
    trace!("user_views: {user_views:#?}");
    let resume = client
        .get_user_items_resume(&GetResumeQuery {
            user_id: user_id.into(),
            limit: 16.into(),
            enable_user_data: true.into(),
            image_type_limit: 1.into(),
            enable_image_types: "Thumb, Backdrop, Primary".into(),
            media_types: "Video".into(),

            fields: "Overview".into(),
            enable_total_record_count: true.into(),
            enable_images: true.into(),
            exclude_active_sessions: false.into(),
            ..Default::default()
        })
        .await
        .context("fetching resumes")?
        .deserialize()
        .await
        .context("deserializing resumes")?;
    trace!("resume: {resume:#?}");
    let next_up = client
        .get_shows_next_up(&GetNextUpQuery {
            user_id: Some(user_id),
            limit: Some(16),
            enable_user_data: Some(true),
            enable_images: Some(true),
            fields: "Overview".into(),
            image_type_limit: Some(1),
            enable_image_types: Some("Thumb, Backdrop, Primary"),
            enable_total_record_count: Some(true),
            disable_first_episode: Some(true),
            enable_resumable: Some(false),
            enable_rewatching: Some(false),
            ..Default::default()
        })
        .await
        .context("fetching next up")?
        .deserialize()
        .await
        .context("deserializing next up")?;
    trace!("next up: {next_up:#?}");
    let latest: HashMap<_, _> = stream::iter(user_views.items.iter())
        .filter_map(async |view| {
            if view.view_type == UserViewType::CollectionFolder {
                match client
                    .get_user_library_latest_media(&GetLatestQuery {
                        user_id: Some(user_id),
                        limit: Some(16),
                        enable_user_data: Some(true),
                        enable_images: Some(true),
                        image_type_limit: Some(1),
                        fields: "Overview".into(),
                        enable_image_types: Some("Thumb, Backdrop, Primary"),
                        parent_id: Some(&view.id),
                        group_items: Some(true),
                        ..Default::default()
                    })
                    .await
                    .with_context(|| format!("fetching latest media from {}", view.name))
                {
                    Ok(items) => match items.deserialize().await {
                        Ok(items) => Some(Ok((view.id.clone(), items))),
                        Err(e) => Some(Err(e.into())),
                    },
                    Err(e) => Some(Err(e)),
                }
            } else {
                None
            }
        })
        .try_collect()
        .await
        .context("fetching latest media")?;
    trace!("recent_grouped: {latest:#?}");
    debug!("collected main screen information");
    Ok(HomeScreenData {
        resume: resume.items,
        next_up: next_up.items,
        views: user_views.items,
        latest,
    })
}

#[instrument(skip_all)]
pub async fn load_home_screen(cx: Pin<&mut TuiContext>) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        "Loading home screen",
        async {
            match load_data(jellyfin, &jellyfin.get_auth().user.id)
                .await
                .context("Loading home screen data")
            {
                Err(e) => Ok(Navigation::Push {
                    current: NextScreen::LoadHomeScreen,
                    next: NextScreen::Error(e),
                }),
                Ok(data) => Ok(Navigation::Replace(NextScreen::HomeScreen(data))),
            }
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}
