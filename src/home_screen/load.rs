use std::{collections::HashMap, pin::pin};

use color_eyre::{eyre::Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use futures_util::{stream, StreamExt, TryStreamExt};
use jellyfin::{
    items::{GetNextUpQuery, GetResumeQuery, MediaItem},
    sha::Sha256,
    user_library::GetLatestQuery,
    user_views::{GetUserViewsQuery, UserView, UserViewType},
    Auth, JellyfinClient,
};
use ratatui::widgets::{Block, Paragraph};
use tracing::{debug, instrument, trace};

use crate::{NextScreen, TuiContext};

pub struct HomeScreenData {
    pub resume: Vec<MediaItem>,
    pub next_up: Vec<MediaItem>,
    pub views: Vec<UserView>,
    pub latest: HashMap<String, Vec<MediaItem>>,
}

#[instrument(skip_all)]
pub async fn load_data(
    client: &JellyfinClient<Auth, impl Sha256>,
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
            user_id: Some(user_id),
            limit: Some(16),
            enable_user_data: Some(false),
            image_type_limit: Some(1),
            enable_image_types: Some("Primary, Backdrop, Thumb"),
            media_types: Some("Video"),
            enable_total_record_count: Some(true),
            enable_images: Some(true),
            exclude_active_sessions: Some(false),
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
            enable_user_data: Some(false),
            enable_images: Some(true),
            image_type_limit: Some(1),
            enable_image_types: Some("Primary, Backdrop, Thumb"),
            enable_total_record_count: Some(true),
            disable_first_episode: Some(true),
            enable_resumable: Some(false),
            enable_rewatching: Some(false),
            ..Default::default()
        })
        .await
        .context("fetching next up")?.deserialize().await.context("deserializing next up")?;
    trace!("next up: {next_up:#?}");
    let latest: HashMap<_, _> = stream::iter(user_views.items.iter())
        .filter_map(async |view| {
            if view.view_type == UserViewType::CollectionFolder {
                match client
                    .get_user_library_latest_media(&GetLatestQuery {
                        user_id: Some(user_id),
                        limit: Some(16),
                        enable_user_data: Some(false),
                        enable_images: Some(true),
                        image_type_limit: Some(1),
                        enable_image_types: Some("Primary, Backdrop, Thumb"),
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
pub async fn load_home_screen(cx: &mut TuiContext) -> Result<NextScreen> {
    let msg = Paragraph::new("Loading home screen")
        .centered()
        .block(Block::bordered());
    let mut load = pin!(load_data(&cx.jellyfin, &cx.jellyfin.get_auth().user.id));
    cx.term
        .draw(|frame| frame.render_widget(&msg, frame.area()))
        .context("rendering ui")?;
    loop {
        tokio::select! {
            data = &mut load => {
                break Ok(NextScreen::HomeScreen(data.context("loading home screen data")?))
            }
            term = cx.events.next() => {
                match term {
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('q')| KeyCode::Esc,
                        modifiers: _,
                        kind: KeyEventKind::Press,
                        state: _,
                    })))
                        | None => break Ok(NextScreen::Quit),
                    Some(Ok(_)) => {
                        cx.term
                          .draw(|frame| frame.render_widget(&msg, frame.area()))
                          .context("rendering ui")?;
                    }
                    Some(Err(e)) => break Err(e).context("Error getting key events from terminal"),
                }
            }
        }
    }
}
