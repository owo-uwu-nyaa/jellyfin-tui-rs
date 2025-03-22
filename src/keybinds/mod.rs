pub mod parse_config;
pub mod stream;
pub mod widget;

use color_eyre::{eyre::Context, Result};
use crossterm::event::{EventStream, KeyCode};
use parse_config::Config;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    path::Path,
    sync::Arc,
};

use crate::{
    home_screen::HomeScreenCommand, login::LoginInfoCommand, mpv::MpvCommand, state::ErrorCommand,
    user_view::UserViewCommand,
};

pub trait Command: Clone + Copy + Debug {
    fn name(self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
}

#[derive(PartialEq, Eq)]
pub struct Key {
    inner: KeyCode,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.to_string().cmp(&other.inner.to_string())
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}
impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}
impl From<KeyCode> for Key {
    fn from(value: KeyCode) -> Self {
        Key { inner: value }
    }
}

pub type BindingMap<T> = Arc<BTreeMap<Key, KeyBinding<T>>>;

#[derive(Debug, Clone)]
pub enum KeyBinding<T: Command> {
    Command(T),
    Group { map: BindingMap<T>, name: String },
    Invalid(String),
}

#[derive(Debug)]
pub struct Keybinds {
    pub fetch_mpv: BindingMap<LoadingCommand>,
    pub play_mpv: BindingMap<MpvCommand>,
    pub fetch_user_view: BindingMap<LoadingCommand>,
    pub user_view: BindingMap<UserViewCommand>,
    pub fetch_home_screen: BindingMap<LoadingCommand>,
    pub home_screen: BindingMap<HomeScreenCommand>,
    pub fetch_login: BindingMap<LoadingCommand>,
    pub login_info: BindingMap<LoginInfoCommand>,
    pub error: BindingMap<ErrorCommand>,
}

impl Keybinds {
    pub fn from_config(config: &Config, strict: bool) -> Result<Self> {
        Ok(Self {
            fetch_mpv: config
                .parse("fetch_mpv", strict)
                .context("in map fetch_mpv")?,
            play_mpv: config
                .parse("play_mpv", strict)
                .context("in map play_mpv")?,
            fetch_user_view: config
                .parse("fetch_user_view", strict)
                .context("in map fetch_user_view")?,
            user_view: config
                .parse("user_view", strict)
                .context("in map user_view")?,
            fetch_home_screen: config
                .parse("fetch_home_screen", strict)
                .context("in map fetch_home_screen")?,
            home_screen: config
                .parse("home_screen", strict)
                .context("in map home_screen")?,
            fetch_login: config
                .parse("fetch_login", strict)
                .context("in map fetch_login")?,
            login_info: config
                .parse("login_info", strict)
                .context("in map login_info")?,
            error: config.parse("error", strict).context("in map error")?,
        })
    }
    pub fn from_str(config: impl AsRef<str>, strict: bool) -> Result<Self> {
        let config = toml::from_str(config.as_ref()).context("de-serializing keybinds")?;
        Keybinds::from_config(&config, strict).context("checking keybinds")
    }
    pub fn from_file(config: impl AsRef<Path>, strict: bool) -> Result<Self> {
        let config = std::fs::read_to_string(config).context("reading keybinds file")?;
        Self::from_str(&config, strict)
    }
}

pub fn check_file(file: impl AsRef<Path>) -> Result<()> {
    Keybinds::from_file(file, true)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum LoadingCommand {
    Quit,
}
impl Command for LoadingCommand {
    fn name(self) -> &'static str {
        "quit"
    }

    fn from_name(name: &str) -> Option<Self> {
        if name == "quit" {
            Self::Quit.into()
        } else {
            None
        }
    }
}

pub enum Text {
    Char(char),
    Str(String),
}

pub enum KeybindEvent<T: Command> {
    Render,
    Command(T),
    Text(Text),
}

pub struct KeybindEvents {
    events: EventStream,
    finished: bool,
}

impl KeybindEvents {
    pub fn new() -> Result<Self> {
        Ok(Self {
            events: EventStream::new(),
            finished: false,
        })
    }
}

pub struct KeybindEventStream<'e, T: Command> {
    inner: &'e mut KeybindEvents,
    top: BindingMap<T>,
    current: Option<BindingMap<T>>,
    text_input: bool,
    current_view: usize,
}

impl<'e, T: Command> KeybindEventStream<'e, T> {
    pub fn new(events: &'e mut KeybindEvents, map: BindingMap<T>) -> Self {
        Self {
            inner: events,
            top: map,
            current: None,
            text_input: false,
            current_view: 0,
        }
    }
    pub fn set_text_input(&mut self, text_input: bool) {
        self.text_input = text_input;
    }
}

#[cfg(test)]
mod tests {
    use super::Keybinds;
    use color_eyre::Result;
    #[test]
    fn check_default_keybinds() -> Result<()> {
        Keybinds::from_str(include_str!("../../config/keybinds.toml"), true)?;
        Ok(())
    }
}
