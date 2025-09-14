use std::{path::PathBuf, str::FromStr};

use color_eyre::eyre::{Context, OptionExt, Result};
use jellyfin_tui_core::config::Config;
use libmpv::MpvProfile;
use serde::Deserialize;
use tracing::{info, instrument};

pub use cache::cache;
pub use keybinds::check_keybinds_file;

mod cache;
mod keybinds;
#[derive(Debug, Deserialize)]
struct ParseConfig {
    pub login_file: Option<PathBuf>,
    pub keybinds_file: Option<PathBuf>,
    pub hwdec: String,
    pub mpv_profile: Option<String>,
    pub mpv_log_level: String,
}

#[instrument]
pub fn init_config(config_file: Option<PathBuf>) -> Result<Config> {
    let (config_dir, config_file) = if let Some(config_file) = config_file {
        (
            config_file
                .parent()
                .ok_or_eyre("config file has no parent directory")?
                .to_path_buf(),
            config_file,
        )
    } else {
        let mut config_dir = dirs::config_dir().ok_or_eyre("Couldn't determine user config dir")?;
        config_dir.push("jellyfin-tui-rs");
        let mut config_file = config_dir.clone();
        config_file.push("config.toml");
        (config_dir, config_file)
    };
    info!("loading config from {}", config_file.display());

    let config: ParseConfig = if config_file.exists() {
        toml::from_str(&std::fs::read_to_string(config_file).context("reading config file")?)
    } else {
        toml::from_str(include_str!("../config.toml"))
    }
    .context("parsing config")?;

    let keybinds = if let Some(keybinds_file) = config.keybinds_file {
        let keybinds = if keybinds_file.is_absolute() {
            keybinds_file
        } else {
            let mut file = config_dir.clone();
            file.push(keybinds_file);
            file
        };
        keybinds::from_file(keybinds, false)
    } else {
        keybinds::from_str(include_str!("../keybinds.toml"), false)
    }
    .context("parsing keybindings")?;

    let mpv_profile = config
        .mpv_profile
        .as_deref()
        .map(MpvProfile::from_str)
        .unwrap_or(Ok(MpvProfile::default()))
        .context("parsing mpv_profile")?;

    let login_file = if let Some(login_file) = config.login_file {
        if login_file.is_absolute() {
            login_file
        } else {
            let mut file = config_dir;
            file.push(&login_file);
            file
        }
    } else {
        let mut login_file = config_dir;
        login_file.push("login.toml");
        login_file
    };

    Ok(Config {
        login_file,
        hwdec: config.hwdec,
        keybinds,
        mpv_log_level: config.mpv_log_level,
        mpv_profile,
    })
}

#[cfg(test)]
mod tests {
    use crate::init_config;
    use color_eyre::Result;
    #[test]
    fn check_default_config() -> Result<()> {
        init_config(None)?;
        Ok(())
    }
}
