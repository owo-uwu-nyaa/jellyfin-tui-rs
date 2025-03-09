mod cache;
mod entry;
mod home_screen;
mod image;
mod login;
mod mpv;
mod state;

use std::{fs::File, io::stdout, path::PathBuf, sync::Mutex};

use clap::Parser;
use color_eyre::eyre::{Context, OptionExt, Result, eyre};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste, EventStream},
    execute,
};
use image::ImageProtocolCache;
use jellyfin::{Auth, JellyfinClient};
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use rayon::ThreadPoolBuilder;
use serde::Deserialize;
use sqlx::SqlitePool;
use state::State;
use tokio::sync::oneshot;
use tracing::{error, info, instrument, level_filters::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[instrument(skip_all)]
async fn run_app(mut term: DefaultTerminal, config: Config, cache: SqlitePool) -> Result<()> {
    let picker = Picker::from_query_stdio().context("getting information for image display")?;
    let mut events = EventStream::new();
    if let Some(client) = login::login(&mut term, &config, &mut events).await? {
        let mut context = TuiContext {
            jellyfin: client,
            term,
            config,
            events,
            image_picker: picker,
            cache,
            image_cache: ImageProtocolCache::new(),
        };
        let mut state = State::new();
        while let Some(screen) = state.pop() {
            state.navigate(screen.show(&mut context).await?);
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

fn main() -> Result<()> {
    std::env::set_var("LC_NUMERIC", "C");
    color_eyre::install().expect("installing color eyre format handler");
    let mut logfile = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or_eyre("unable to determine runtime or cache dir")?;
    logfile.push("jellyfin-tui-rs.log");
    let format = tracing_subscriber::fmt::format();
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("parsing log config from RUST_LOG failed");
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
    #[cfg(feature = "attach")]
    debug();
    let config = init()?;
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
        .thread_name(|n|format!("tui-worker-{n}"))
        .build_global()
        .context("building global thread pool")?;
    info!("logging initiaited");
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    config_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Config {
    pub login_file: PathBuf,
    pub hwdec: String,
    pub mpv_log_level: String,
}

#[instrument]
fn init() -> Result<Config> {
    let mut config_dir = dirs::config_dir().ok_or_eyre("Couldn't determine user config dir")?;
    config_dir.push("jellyfin-tui-rs");
    let args = Args::try_parse()?;
    let config_file = args.config_file.unwrap_or_else(|| {
        let mut file = config_dir.to_path_buf();
        file.push("config.toml");
        file
    });
    info!("loading config from {}", config_dir.display());
    let mut login_file = config_dir.to_path_buf();
    login_file.push("login.toml");
    let mut config = config::Config::builder()
        .set_default(
            "login_file",
            login_file
                .to_str()
                .ok_or_eyre("non unicode char in config dir")?,
        )?
        .set_default("hwdec", "auto-safe")?
    .set_default("mpv_log_level", "info")?;
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

struct TuiContext {
    pub jellyfin: JellyfinClient<Auth>,
    pub term: DefaultTerminal,
    pub config: Config,
    pub events: EventStream,
    pub image_picker: Picker,
    pub cache: SqlitePool,
    pub image_cache: ImageProtocolCache,
}
