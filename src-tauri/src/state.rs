use crate::domain::ScanStatus;
use notify::RecommendedWatcher;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::{Mutex, RwLock, Semaphore};

pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub scans: Arc<RwLock<HashMap<String, ScanStatus>>>,
    pub pauses: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pub watchers: std::sync::Mutex<HashMap<String, RecommendedWatcher>>,
    pub thumbnail_workers: Arc<Semaphore>,
    pub thumbnail_pending: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub async fn initialize(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
        let database_path = data_dir.join("volum.sqlite3");
        if database_path.exists() {
            let backup = data_dir.join("volum.sqlite3.pre-migration.bak");
            let _ = std::fs::copy(&database_path, backup);
        }
        let options = SqliteConnectOptions::from_str(database_path.to_string_lossy().as_ref())
            .map_err(|error| error.to_string())?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
            .map_err(|error| error.to_string())?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            pool,
            data_dir: data_dir.to_path_buf(),
            scans: Arc::new(RwLock::new(HashMap::new())),
            pauses: Arc::new(RwLock::new(HashMap::new())),
            watchers: std::sync::Mutex::new(HashMap::new()),
            thumbnail_workers: Arc::new(Semaphore::new(2)),
            thumbnail_pending: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initializes_database_and_fts() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path()).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_search")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
