use std::{future::Future, time::Duration};

use sqlx::{
    SqlitePool, query,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};
use tokio::{
    select,
    time::{Instant, MissedTickBehavior, interval_at},
};
use tracing::{Instrument, error, info, info_span, instrument};

#[instrument]
async fn open_db() -> Result<SqlitePool> {
    let mut db_path = dirs::cache_dir().ok_or_eyre("unable to detect cache dir")?;
    db_path.push("jellyfin-tui.sqlite");
    let create = async || {
        info!("opening sqlite db at {}", db_path.display());
        SqlitePoolOptions::new()
            .min_connections(0)
            .max_connections(2)
            .acquire_time_level(log::LevelFilter::Debug)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&db_path)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .create_if_missing(true)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Off),
            )
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
    mut f: impl FnMut(SqlitePool) -> Fut,
    db: SqlitePool,
) {
    let mut interval = interval_at(
        Instant::now() + Duration::from_secs(30),
        Duration::from_secs(60 * 60),
    );
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    select! {
        biased;
        _ = db.close_event() => {
            return
        }
        _ = interval.tick() => {}
    }
    if let Err(err) = f(db.clone()).await {
        error!("Error maintaining cache: {err:?}")
    }
}

#[instrument(skip_all)]
pub async fn cache() -> Result<SqlitePool> {
    let db = open_db().await?;
    let migrate = info_span!("migrate");
    sqlx::migrate!("../migrations")
        .run(&db)
        .instrument(migrate.clone())
        .await?;
    migrate.in_scope(|| info!("migrations applied"));
    let maintainance = info_span!("cache_maintainance");
    tokio::spawn(cache_maintainance(clean_images, db.clone()).instrument(maintainance.clone()));
    tokio::spawn(cache_maintainance(clean_creds, db.clone()).instrument(maintainance));
    Ok(db)
}

#[instrument]
pub async fn clean_creds(db: SqlitePool) -> Result<()> {
    let res = query!("delete from creds where (added+30*24*60*60)<unixepoch()")
        .execute(&db)
        .await
        .context("deleting old creds")?;
    if res.rows_affected() > 0 {
        info!("removed {} access tokens from cache", res.rows_affected());
    }
    Ok(())
}

#[instrument]
pub async fn clean_images(db: SqlitePool) -> Result<()> {
    let res = query!("delete from image_cache where (added+7*24*60*60)<unixepoch()")
        .execute(&db)
        .await
        .context("deleting old images from cache")?;
    if res.rows_affected() > 0 {
        info!("removed {} images from cache", res.rows_affected());
    }
    Ok(())
}
