use std::path::Path;

use color_eyre::{Result, eyre::Context};

use crate::keybind_defs::Keybinds;

pub fn check_keybinds_file(file: impl AsRef<Path>) -> Result<()> {
    from_file(
        file,
        true,
        from_str(include_str!("../keybinds.toml"), true)?.0,
    )?;
    Ok(())
}

pub fn from_str(config: impl AsRef<str>, strict: bool) -> Result<(Keybinds, Vec<String>)> {
    let config = toml::from_str(config.as_ref()).context("de-serializing keybinds")?;
    let binds = Keybinds::from_config(&config, strict).context("checking keybinds")?;
    Ok((binds, config.help_prefixes))
}

pub fn from_file(
    config: impl AsRef<Path>,
    strict: bool,
    default: Keybinds,
) -> Result<(Keybinds, Vec<String>)> {
    let config = std::fs::read_to_string(config).context("reading keybinds file")?;
    let config = toml::from_str(config.as_ref()).context("de-serializing keybinds")?;
    let binds = Keybinds::from_config_with_default(&config, strict, default)
        .context("checking keybinds")?;
    Ok((binds, config.help_prefixes))
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
