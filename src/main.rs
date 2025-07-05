mod cache;
mod entry;
mod error;
mod fetch;
mod grid;
mod home_screen;
mod image;
mod item_details;
mod item_list_details;
mod keybinds;
mod list;
mod login;
mod mpv;
mod screen;
mod state;
mod user_view;

use std::{
    fs::File,
    io::{stdout, Write},
    path::PathBuf,
    pin::pin,
    sync::Mutex,
};

use ::keybinds::KeybindEvents;
use clap::{Parser, Subcommand};
use color_eyre::eyre::{eyre, Context, OptionExt, Result};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
};
use image::ImageProtocolCache;
use jellyfin::{socket::JellyfinWebSocket, Auth, JellyfinClient};
use keybinds::Keybinds;
use pin_project_lite::pin_project;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use rayon::ThreadPoolBuilder;
use serde::Deserialize;
use sqlx::SqlitePool;
use state::State;
use tokio::sync::oneshot;
use tracing::{error, info, instrument, level_filters::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[instrument(skip_all)]
async fn run_app(mut term: DefaultTerminal, config: Config, cache: SqlitePool) -> Result<()> {
    let picker = Picker::from_query_stdio().context("getting information for image display")?;
    let mut events = KeybindEvents::new()?;
    if let Some(client) = login::login(&mut term, &config, &mut events, &cache).await? {
        let jellyfin_socket = client.get_socket();
        let context = TuiContext {
            jellyfin: client,
            jellyfin_socket,
            term,
            config,
            events,
            image_picker: picker,
            cache,
            image_cache: ImageProtocolCache::new(),
        };
        let mut context = pin!(context);
        let mut state = State::new();
        while let Some(screen) = state.pop() {
            state.navigate(screen.show(context.as_mut()).await?);
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
#[instrument(skip_all)]
async fn run(term: DefaultTerminal, config: Config, paniced: oneshot::Receiver<()>) -> Result<()> {
    let cache = cache::initialize_cache().await?;
    let res = tokio::select! {
        res = tokio::spawn(run_app(term, config, cache.clone())) => {
            match res.context("joining main task"){
                Ok(v)=>v,
                Err(e)=>Err(e)
            }
        }
        res = paniced => {
            Err(
                res.context("failed to receive panic notification")
                   .err()
                   .unwrap_or_else(||eyre!("thread pool task paniced"))
            )
        }
    };
    cache.close().await;
    res
}

#[allow(unused)]
fn debug() {
    info!("pid is {}", std::process::id());
    #[cfg(target_os = "linux")]
    unsafe {
        libc::prctl(libc::PR_SET_PTRACER, libc::PR_SET_PTRACER_ANY);
    }
}

fn log_stdout() -> Result<()> {
    let format = tracing_subscriber::fmt::format();
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .context("parsing log config from RUST_LOG")?;
    let fmt_layer = tracing_subscriber::fmt::layer()
        .event_format(format)
        .with_filter(filter);
    let error_layer = ErrorLayer::default();
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(error_layer)
        .try_init()
        .context("initializing tracing subscriber")?;
    Ok(())
}

fn log_file() -> Result<()> {
    let mut logfile = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or_eyre("unable to determine runtime or cache dir")?;
    logfile.push("jellyfin-tui-rs.log");
    let format = tracing_subscriber::fmt::format();
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .context("parsing log config from RUST_LOG")?;
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(Mutex::new(
            File::create(&logfile).context("opening logfile")?,
        ))
        .event_format(format)
        .with_filter(filter);

    let error_layer = ErrorLayer::default();
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(error_layer)
        .init();
    println!("logging to {}", logfile.display());
    Ok(())
}

fn main() -> Result<()> {
    std::env::set_var("LC_NUMERIC", "C");
    color_eyre::install().expect("installing color eyre format handler");
    let args = Args::try_parse()?;
    match args.action {
        Some(Action::Print { what }) => {
            match what {
                PrintAction::ConfigDir => println!(
                    "{}",
                    dirs::config_dir()
                        .ok_or_eyre("Couldn't determine user config dir")?
                        .display()
                ),
                PrintAction::Keybinds => {
                    stdout().write_all(include_str!("../config/keybinds.toml").as_bytes())?
                }
                PrintAction::Config => {
                    stdout().write_all(include_str!("../config/config.toml").as_bytes())?
                }
            }
            Ok(())
        }
        Some(Action::CheckKeybinds { file }) => {
            log_stdout()?;
            keybinds::check_file(file)
        }
        None => {
            log_file()?;
            #[cfg(feature = "attach")]
            debug();
            let config = init_config(args.config_file)?;
            let (send_panic, paniced) = oneshot::channel();
            let send_panic = Mutex::new(Some(send_panic));
            ThreadPoolBuilder::new()
                .panic_handler(move |_| {
                    error!("panic in thread pool");
                    send_panic
                        .lock()
                        .expect("taking lock failed")
                        .take()
                        .into_iter()
                        .for_each(|send_panic| send_panic.send(()).expect("sending panic failed"));
                })
                .thread_name(|n| format!("tui-worker-{n}"))
                .build_global()
                .context("building global thread pool")?;
            let term = ratatui::init();
            let hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                execute!(stdout(), DisableBracketedPaste).expect("resetting bracket paste failed");
                hook(panic)
            }));
            execute!(stdout(), EnableBracketedPaste)
                .context("enabling bracket paste")
                .expect("failed to enable bracket paste");

            let res = run(term, config, paniced);
            execute!(stdout(), DisableBracketedPaste).expect("resetting bracket paste failed");
            ratatui::restore();
            res
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    action: Option<Action>,
    /// alternative config file
    config_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Action {
    CheckKeybinds {
        /// keybinds config to check
        file: PathBuf,
    },
    Print {
        /// what should be printed
        #[command(subcommand)]
        what: PrintAction,
    },
}

#[derive(Debug, Subcommand)]
enum PrintAction {
    ConfigDir,
    Keybinds,
    Config,
}

#[derive(Debug, Deserialize)]
struct ParseConfig {
    pub login_file: Option<PathBuf>,
    pub keybinds_file: Option<PathBuf>,
    pub hwdec: String,
    pub mpv_log_level: String,
}

#[derive(Debug)]
struct Config {
    pub login_file: PathBuf,
    pub hwdec: String,
    pub keybinds: Keybinds,
    pub mpv_log_level: String,
}

#[instrument]
fn init_config(config_file: Option<PathBuf>) -> Result<Config> {
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
        toml::from_str(include_str!("../config/config.toml"))
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
        Keybinds::from_file(keybinds, false)
    } else {
        Keybinds::from_str(include_str!("../config/keybinds.toml"), false)
    }
    .context("parsing keybindings")?;

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

pin_project! {
    struct TuiContext {
        pub jellyfin: JellyfinClient<Auth>,
        #[pin]
        pub jellyfin_socket: JellyfinWebSocket,
        pub term: DefaultTerminal,
        pub config: Config,
        pub events: KeybindEvents,
        pub image_picker: Picker,
        pub cache: SqlitePool,
        pub image_cache: ImageProtocolCache,
    }
}
