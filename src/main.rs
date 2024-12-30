mod login;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::{Context, OptionExt, Result};
use crossterm::event::EventStream;
use ratatui::DefaultTerminal;
use serde::Deserialize;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let res = run().await;
    ratatui::restore();
    res
}

async fn run() -> Result<()> {
    let (mut term, config) = init()?;
    let mut events = EventStream::new();
    if let Some(client) = login::login(&mut term, &config, &mut events).await? {}

    println!("Hello, world!");
    Ok(())
}
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    config_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Config {
    pub login_file: PathBuf,
}

fn init() -> Result<(DefaultTerminal, Config)> {
    color_eyre::install()?;
    let mut config_dir = dirs::config_dir().ok_or_eyre("Couldn't determine user config dir")?;
    config_dir.push("jellyfin-tui-rs");
    let args = Args::try_parse()?;
    let config_file = args.config_file.unwrap_or_else(|| {
        let mut file = config_dir.to_path_buf();
        file.push("config.toml");
        file
    });
    let mut login_file = config_dir.to_path_buf();
    login_file.push("login.toml");
    let mut config = config::Config::builder().set_default(
        "login_file",
        login_file
            .to_str()
            .ok_or_eyre("non unicode char in config dir")?,
    )?;
    if let Ok(file) = std::fs::read_to_string(config_file) {
        config = config.add_source(config::File::from_str(&file, config::FileFormat::Toml));
    }
    let config = config
        .add_source(config::Environment::with_prefix("JELLY_TUI"))
        .build()
        .context("building config")?
        .try_deserialize()
        .context("collecting config")?;

    let term = ratatui::try_init()?;
    Ok((term, config))
}
