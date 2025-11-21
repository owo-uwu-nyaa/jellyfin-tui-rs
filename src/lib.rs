use std::{
    path::PathBuf,
    pin::{Pin, pin}, sync::Arc,
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
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use sqlx::SqliteConnection;
use tokio_util::sync::CancellationToken;
use tracing::{error_span, instrument};
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
        NextScreen::MkPlayer { items, index } => player::mk_player(cx, items, index),
        NextScreen::Play(player_handle) => player::play(cx, player_handle).await,
        NextScreen::Error(report) => {
            let cx = cx.project();
            error::display_error(cx.term, cx.events, &cx.config.keybinds, report).await
        }
        NextScreen::ItemDetails(media_item) => {
            item_view::item_details::display_item(cx, media_item).await
        }
        NextScreen::ItemListDetailsData(media_item, media_items) => {
            item_view::item_list_details::handle_item_list_details_data(cx, media_item, media_items)
        }
        NextScreen::ItemListDetails(media_item, entry_list, images_available) => {
            item_view::item_list_details::display_item_list_details(cx, media_item, entry_list, images_available)
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
    }
}

async fn login_jellyfin(
    term: &mut DefaultTerminal,
    events: &mut KeybindEvents,
    config: &Config,
    cache: &tokio::sync::Mutex<SqliteConnection>,
) -> Result<Option<(JellyfinClient, JellyfinWebSocket)>> {
    Ok(
        if let Some(client) = login::login(term, config, events, cache).await? {
            let socket = client.get_socket()?;
            Some((client, socket))
        } else {
            None
        },
    )
}

#[instrument(skip_all, level="debug")]
async fn login(
    term: &mut DefaultTerminal,
    events: &mut KeybindEvents,
    config: &Config,
    cache: &tokio::sync::Mutex<SqliteConnection>,
) -> Option<(JellyfinClient, JellyfinWebSocket)> {
    loop {
        match login_jellyfin(term, events, config, cache).await {
            Ok(v) => break v,
            Err(e) => match error::display_error(term, events, &config.keybinds, e).await {
                Err(_) | Ok(Navigation::Exit) => break None,
                _ => {}
            },
        }
    }
}

#[instrument(skip_all, level="debug")]
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
    config: Config,
    cache: Arc<tokio::sync::Mutex<SqliteConnection>>,
    image_picker: Picker,
) {
    if let Some((jellyfin, jellyfin_socket)) = login(&mut term, &mut events, &config, &cache).await
    {
        let cx = pin!(TuiContext {
            jellyfin,
            jellyfin_socket,
            term,
            config,
            events,
            image_picker: Arc::new(image_picker),
            cache,
            image_cache: ImageProtocolCache::new()
        });
        run_state(cx).await
    }
}

#[instrument(skip_all, level="debug")]
#[tokio::main(flavor = "current_thread")]
pub async fn run_app(
    term: DefaultTerminal,
    cancel: CancellationToken,
    config_file: Option<PathBuf>,
) -> Result<()> {
    let cache = config::cache().await?;
    let config = init_config(config_file)?;
    let image_picker =
        Picker::from_query_stdio().context("getting information for image display")?;
    let events = KeybindEvents::new()?;
    spawn::run_with_spawner(
        run_app_inner(term, events, config, cache.clone(), image_picker),
        cancel,
        error_span!("jellyfin-tui"),
    )
    .await;
    Ok(())
}
