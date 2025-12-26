use std::{future::Future, ops::DerefMut, sync::Arc, time::Duration};

use sqlx::{ConnectOptions, SqliteConnection, query, sqlite::SqliteConnectOptions};

use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};
use tokio::{
    sync::Mutex,
    time::{MissedTickBehavior, interval},
};
use tracing::{Instrument, error, info, info_span, instrument};

#[instrument]
async fn open_db() -> Result<SqliteConnection> {
    let mut db_path = dirs::cache_dir().ok_or_eyre("unable to detect cache dir")?;
    db_path.push("jellyfin-tui.sqlite");
    let create = async || {
        info!("opening sqlite db at {}", db_path.display());
        SqliteConnectOptions::new()
            .filename(&db_path)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .create_if_missing(true)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Off)
            .pragma("foreign_keys", "ON")
            .connect()
            .await
    };
    match create().await {
        Ok(db) => Ok(db),
        Err(e) => {
            tracing::error!("error opening db: {e:?}");
            info!("cache db might be corrupted. deleting...");
            std::fs::remove_file(&db_path).context("removing corrupted db")?;
            create().await.context("creating new db")
        }
    }
}

async fn cache_maintainance<Fut: Future<Output = Result<()>>>(
    mut f: impl FnMut(Arc<Mutex<SqliteConnection>>) -> Fut,
    db: Arc<Mutex<SqliteConnection>>,
) {
    let mut interval = interval(Duration::from_secs(60 * 60));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(err) = f(db.clone()).await {
            error!("Error maintaining cache: {err:?}")
        }
    }
}

#[instrument(skip_all)]
pub async fn cache() -> Result<Arc<Mutex<SqliteConnection>>> {
    let mut db = open_db().await?;
    let migrate = info_span!("migrate");
    sqlx::migrate!("../migrations")
        .run(&mut db)
        .instrument(migrate.clone())
        .await?;
    migrate.in_scope(|| info!("migrations applied"));
    let maintainance = info_span!("cache_maintainance");
    let db = Arc::new(Mutex::new(db));
    tokio::spawn(cache_maintainance(clean_images, db.clone()).instrument(maintainance.clone()));
    Ok(db)
}

#[instrument]
pub async fn clean_images(db: Arc<Mutex<SqliteConnection>>) -> Result<()> {
    let res = query!("delete from image_cache where (added+7*24*60*60)<unixepoch()")
        .execute(db.lock().await.deref_mut())
        .await
        .context("deleting old images from cache")?;
    if res.rows_affected() > 0 {
        info!("removed {} images from cache", res.rows_affected());
    }
    Ok(())
}
