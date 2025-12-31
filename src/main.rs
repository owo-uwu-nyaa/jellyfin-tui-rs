use std::{
    fs::File,
    io::{Write, stdout},
    path::PathBuf,
    sync::Mutex,
};

use clap::{Parser, Subcommand};
use color_eyre::eyre::{Context, OptionExt, Result};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
};
use jellyhaj::run_app;
use rayon::ThreadPoolBuilder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

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
    logfile.push("jellyhaj.log");
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
        .with(tui_logger::TuiTracingSubscriberLayer)
        .init();
    println!("logging to {}", logfile.display());
    Ok(())
}

fn main() -> Result<()> {
    unsafe { std::env::set_var("LC_NUMERIC", "C") };
    let args = Args::parse();
    if args.features {
        println!("enabled features: {}", env!("JELLYFIN_TUI_FEATURES"));
        return Ok(());
    }
    match args.action {
        Some(Action::Print { what }) => {
            color_eyre::install().expect("installing color eyre format handler");
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
            color_eyre::install().expect("installing color eyre format handler");
            log_stdout()?;
            config::check_keybinds_file(file)
        }
        None => {
            log_file()?;
            tui_logger::init_logger(tui_logger::LevelFilter::Debug)
                .context("setting up tui logger")?;
            tui_logger::set_default_level(tui_logger::LevelFilter::Info);
            tui_logger::set_env_filter_from_env(None);
            #[cfg(feature = "attach")]
            debug();
            let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::new().into_hooks();
            eyre_hook.install().expect("installing eyre hook");
            let cancel = CancellationToken::new();
            let handler_cancel = cancel.clone();
            std::panic::set_hook(Box::new(move |panic| {
                handler_cancel.cancel();
                let report = panic_hook.panic_report(panic);
                error!("{}", report);
                eprintln!("{}", report)
            }));
            ThreadPoolBuilder::new()
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

            let res = run_app(term, cancel, args.config, args.use_builtin_config);
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
    #[arg(short, long)]
    config: Option<PathBuf>,
    #[arg(short = 'b', long)]
    use_builtin_config: bool,
    #[arg(short, long)]
    features: bool,
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
