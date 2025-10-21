use std::pin::Pin;

use crate::config::Config;
use ::keybinds::KeybindEvents;
use entries::image::cache::ImageProtocolCache;
use jellyfin::{Auth, JellyfinClient, socket::JellyfinWebSocket};
use player_core::PlayerHandle;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use sqlx::SqlitePool;

pub struct TuiContext {
    pub jellyfin: JellyfinClient<Auth>,
    pub jellyfin_socket: JellyfinWebSocket,
    pub term: DefaultTerminal,
    pub config: Config,
    pub events: KeybindEvents,
    pub image_picker: Picker,
    pub cache: SqlitePool,
    pub image_cache: ImageProtocolCache,
    pub mpv_handle: PlayerHandle,
}

pub struct TuiContextProj<'p> {
    pub jellyfin: &'p JellyfinClient<Auth>,
    pub jellyfin_socket: Pin<&'p mut JellyfinWebSocket>,
    pub term: &'p mut DefaultTerminal,
    pub config: &'p Config,
    pub events: &'p mut KeybindEvents,
    pub image_picker: &'p mut Picker,
    pub cache: &'p SqlitePool,
    pub image_cache: &'p mut ImageProtocolCache,
    pub mpv_handle: &'p mut PlayerHandle,
}

impl TuiContext {
    #[doc(hidden)]
    #[inline]
    pub fn project<'__pin>(self: Pin<&'__pin mut Self>) -> TuiContextProj<'__pin> {
        unsafe {
            let Self {
                jellyfin,
                jellyfin_socket,
                term,
                config,
                events,
                image_picker,
                cache,
                image_cache,
                mpv_handle,
            } = self.get_unchecked_mut();
            TuiContextProj {
                jellyfin,
                jellyfin_socket: Pin::new_unchecked(jellyfin_socket),
                term,
                config,
                events,
                image_picker,
                cache,
                image_cache,
                mpv_handle,
            }
        }
    }
}
