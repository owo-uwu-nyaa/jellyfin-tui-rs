use std::{
    path::PathBuf,
    pin::{Pin, pin},
    sync::Arc,
};

use color_eyre::{Result, eyre::Context};
use config::init_config;
use entries::image::cache::ImageProtocolCache;
use jellyfin::{JellyfinClient, socket::JellyfinWebSocket};
use jellyfin_tui_core::{
    config::Config,
    context::TuiContext,
    state::{Navigation, NextScreen, State},
};
use keybinds::KeybindEvents;
use player_core::OwnedPlayerHandle;
use player_jellyfin::player_jellyfin;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use spawn::Spawner;
use sqlx::SqliteConnection;
use tokio_util::sync::CancellationToken;
use tracing::{error_span, instrument};

use crate::error::ResultDisplayExt;
pub mod error;

async fn show_screen(screen: NextScreen, cx: Pin<&mut TuiContext>) -> Result<Navigation> {
    match screen {
        NextScreen::LoadHomeScreen => home_screen::load::load_home_screen(cx).await,
        NextScreen::HomeScreenData {
            resume,
            next_up,
            views,
            latest,
        } => home_screen::handle_home_screen_data(cx, resume, next_up, views, latest),
        NextScreen::HomeScreen(entry_screen, images_available) => {
            home_screen::display_home_screen(cx, entry_screen, images_available).await
        }
        NextScreen::LoadUserView(user_view) => user_view::fetch_user_view(cx, user_view).await,
        NextScreen::UserView { view, items } => user_view::display_user_view(cx, view, items).await,
        NextScreen::LoadPlayItem(load_play) => {
            player::fetch_items::fetch_screen(cx, load_play).await
        }
        NextScreen::Play { items, index } => player::play(cx, items, index).await,
        NextScreen::Error(report) => {
            let cx = cx.project();
            error::display_error(
                cx.term,
                cx.events,
                &cx.config.keybinds,
                &cx.config.help_prefixes,
                report,
            )
            .await
        }
        NextScreen::ItemDetails(media_item) => {
            item_view::item_details::display_item(cx, media_item).await
        }
        NextScreen::ItemListDetailsData(media_item, media_items) => {
            item_view::item_list_details::handle_item_list_details_data(cx, media_item, media_items)
        }
        NextScreen::ItemListDetails(media_item, entry_list, images_available) => {
            item_view::item_list_details::display_item_list_details(
                cx,
                media_item,
                entry_list,
                images_available,
            )
            .await
        }
        NextScreen::FetchItemListDetails(media_item) => {
            item_view::item_list_details::display_fetch_item_list(cx, media_item).await
        }
        NextScreen::FetchItemListDetailsRef(id) => {
            item_view::item_list_details::display_fetch_item_list_ref(cx, &id).await
        }
        NextScreen::FetchItemDetails(id) => {
            item_view::item_details::display_fetch_item(cx, &id).await
        }
        NextScreen::RefreshItem(item) => refresh_item::show_refresh_item(cx, item).await,
        NextScreen::SendRefreshItem(item, refresh_item_query) => {
            refresh_item::refresh_screen(cx, item, refresh_item_query).await
        }
    }
}

async fn login_jellyfin(
    term: &mut DefaultTerminal,
    events: &mut KeybindEvents,
    config: &Config,
) -> Result<Option<(JellyfinClient, JellyfinWebSocket)>> {
    Ok(
        if let Some(client) = login::login(term, config, events).await? {
            let socket = client.get_socket()?;
            Some((client, socket))
        } else {
            None
        },
    )
}

#[instrument(skip_all, level = "debug")]
async fn login(
    term: &mut DefaultTerminal,
    events: &mut KeybindEvents,
    config: &Config,
) -> Option<(JellyfinClient, JellyfinWebSocket)> {
    loop {
        match login_jellyfin(term, events, config).await {
            Ok(v) => break v,
            Err(e) => {
                match error::display_error(term, events, &config.keybinds, &config.help_prefixes, e)
                    .await
                {
                    Err(_) | Ok(Navigation::Exit) => break None,
                    _ => {}
                }
            }
        }
    }
}

#[instrument(skip_all, level = "debug")]
async fn run_state(mut cx: Pin<&mut TuiContext>) {
    let mut state = State::new();
    while let Some(screen) = state.pop() {
        state.navigate(match show_screen(screen, cx.as_mut()).await {
            Ok(nav) => nav,
            Err(e) => Navigation::Replace(NextScreen::Error(e)),
        });
    }
}

async fn run_app_inner(
    mut term: DefaultTerminal,
    mut events: KeybindEvents,
    spawner: Spawner,
    config: Config,
    cache: Arc<tokio::sync::Mutex<SqliteConnection>>,
    image_picker: Picker,
) {
    if let Some((jellyfin, jellyfin_socket)) = login(&mut term, &mut events, &config).await
        && let Some(mpv_handle) = OwnedPlayerHandle::new(
            jellyfin.clone(),
            &config.hwdec,
            config.mpv_profile,
            &config.mpv_log_level,
            config.mpv_config_file.as_deref(),
            true,
            &spawner,
        )
        .display_error(
            &mut term,
            &mut events,
            &config.keybinds,
            &config.help_prefixes,
        )
        .await
    {
        spawner.spawn(
            player_jellyfin(mpv_handle.clone(), jellyfin.clone(), spawner.clone()),
            error_span!("player_jellyfin"),
        );
        #[cfg(feature = "mpris")]
        spawner.spawn_res(
            player_mpris::run_mpris_service(mpv_handle.clone(), jellyfin.clone()),
            error_span!("player_mpris"),
        );
        let cx = pin!(TuiContext {
            jellyfin,
            jellyfin_socket,
            term,
            config,
            events,
            image_picker: Arc::new(image_picker),
            cache,
            image_cache: ImageProtocolCache::new(),
            mpv_handle,
            stats: Default::default()
        });
        run_state(cx).await
    }
}

#[instrument(skip_all, level = "debug")]
#[tokio::main(flavor = "current_thread")]
pub async fn run_app(
    term: DefaultTerminal,
    cancel: CancellationToken,
    config_file: Option<PathBuf>,
    use_builtin_config: bool,
) -> Result<()> {
    let cache = config::cache().await?;
    let config = init_config(config_file, use_builtin_config)?;
    let image_picker =
        Picker::from_query_stdio().context("getting information for image display")?;
    let events = KeybindEvents::new()?;
    spawn::run_with_spawner(
        |spawner| run_app_inner(term, events, spawner, config, cache.clone(), image_picker),
        cancel,
        error_span!("jellyfin-tui"),
    )
    .await;
    Ok(())
}
