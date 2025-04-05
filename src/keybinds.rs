use std::path::Path;

use color_eyre::{eyre::Context, Result};
use keybinds::{BindingMap, Command};

use crate::{
    error::ErrorCommand, home_screen::HomeScreenCommand, item_details::EpisodeCommand,
    item_list_details::SeasonCommand, login::LoginInfoCommand, mpv::MpvCommand,
    user_view::UserViewCommand,
};

#[derive(Debug)]
#[keybinds::gen_from_config]
pub struct Keybinds {
    pub fetch: BindingMap<LoadingCommand>,
    pub play_mpv: BindingMap<MpvCommand>,
    pub user_view: BindingMap<UserViewCommand>,
    pub home_screen: BindingMap<HomeScreenCommand>,
    pub login_info: BindingMap<LoginInfoCommand>,
    pub error: BindingMap<ErrorCommand>,
    pub item_details: BindingMap<EpisodeCommand>,
    pub item_list_details: BindingMap<SeasonCommand>,
}

impl Keybinds {
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

#[cfg(test)]
mod tests {
    use super::Keybinds;
    use color_eyre::Result;
    #[test]
    fn check_default_keybinds() -> Result<()> {
        Keybinds::from_str(include_str!("../config/keybinds.toml"), true)?;
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Command)]
pub enum LoadingCommand {
    Quit,
}
