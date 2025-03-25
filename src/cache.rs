use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};

use color_eyre::{
    eyre::{Context, OptionExt},
    Result,
};
use tracing::{info, instrument};

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

#[instrument(skip_all)]
pub async fn initialize_cache() -> Result<SqlitePool> {
    let db = open_db().await?;
    sqlx::migrate!().run(&db).await?;
    info!("migrations applied");
    tokio::spawn(crate::image::clean_image_cache(db.clone()));
    tokio::spawn(crate::login::clean_creds(db.clone()));
    Ok(db)
}
