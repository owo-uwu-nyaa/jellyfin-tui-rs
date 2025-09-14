use std::path::Path;

use color_eyre::{Result, eyre::Context};
use jellyfin_tui_core::keybinds::Keybinds;

pub fn check_keybinds_file(file: impl AsRef<Path>) -> Result<()> {
    from_file(file, true)?;
    Ok(())
}

pub fn from_str(config: impl AsRef<str>, strict: bool) -> Result<Keybinds> {
    let config = toml::from_str(config.as_ref()).context("de-serializing keybinds")?;
    Keybinds::from_config(&config, strict).context("checking keybinds")
}

pub fn from_file(config: impl AsRef<Path>, strict: bool) -> Result<Keybinds> {
    let config = std::fs::read_to_string(config).context("reading keybinds file")?;
    from_str(&config, strict)
}

#[cfg(test)]
mod tests {
    use super::Keybinds;
    use crate::keybinds::from_str;
    use color_eyre::Result;
    #[test]
    fn check_default_keybinds() -> Result<()> {
        from_str(include_str!("../keybinds.toml"), true)?;
        Ok(())
    }
    #[test]
    fn check_commands_unique() {
        Keybinds::assert_uniqueness();
    }
}
