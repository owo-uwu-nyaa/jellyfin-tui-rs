mod login;

use std::{io::stdout, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{Context, OptionExt, Result};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste, EventStream},
    execute,
};
use serde::Deserialize;

struct OnDrop<F: FnOnce()> {
    f: Option<F>,
}
impl<F: FnOnce()> OnDrop<F> {
    fn new(f: F) -> Self {
        Self { f: Some(f) }
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        self.f.take().unwrap()()
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut term = ratatui::init();
    let _ = OnDrop::new(ratatui::restore);
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        execute!(stdout(), DisableBracketedPaste).expect("resetting bracket paste failed");
        hook(panic)
    }));
    execute!(stdout(), EnableBracketedPaste).context("enabling bracket paste")?;
    let _ = OnDrop::new(|| {
        execute!(stdout(), DisableBracketedPaste).expect("resetting bracket paste failed")
    });
    let config = init()?;
    let mut events = EventStream::new();
    if let Some(_client) = login::login(&mut term, &config, &mut events).await? {}
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

fn init() -> Result<Config> {
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

    Ok(config)
}
