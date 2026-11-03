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
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub scans: Arc<RwLock<HashMap<String, ScanStatus>>>,
    pub pauses: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pub watchers: Arc<std::sync::Mutex<HashMap<String, RecommendedWatcher>>>,
    pub thumbnail_workers: Arc<Semaphore>,
    pub thumbnail_pending: Arc<Mutex<HashSet<String>>>,
    pub thumbnail_cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    pub backup_lock: Arc<Mutex<()>>,
    pub file_operation_lock: Arc<Mutex<()>>,
}

impl AppState {
    pub async fn initialize(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|error| error.to_string())?;
        let database_path = data_dir.join("volum.sqlite3");
        let database_existed = database_path.exists();
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
        let migrator = sqlx::migrate!("./migrations");
        let latest_migration = migrator
            .iter()
            .map(|migration| migration.version)
            .max()
            .unwrap_or_default();
        let applied_migration = if database_existed {
            sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        } else {
            0
        };
        if database_existed && applied_migration < latest_migration {
            let backup = data_dir.join(format!(
                "volum.sqlite3.pre-migration-v{applied_migration}.bak"
            ));
            if !backup.exists() {
                let escaped = backup.to_string_lossy().replace('\'', "''");
                sqlx::query(&format!("VACUUM INTO '{escaped}'"))
                    .execute(&pool)
                    .await
                    .map_err(|error| error.to_string())?;
            }
        }
        migrator
            .run(&pool)
            .await
            .map_err(|error| error.to_string())?;
        crate::project_storage::recover_pending_operations(&pool, data_dir).await?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self {
            pool,
            data_dir: data_dir.to_path_buf(),
            scans: Arc::new(RwLock::new(HashMap::new())),
            pauses: Arc::new(RwLock::new(HashMap::new())),
            watchers: Arc::new(std::sync::Mutex::new(HashMap::new())),
            thumbnail_workers: Arc::new(Semaphore::new(2)),
            thumbnail_pending: Arc::new(Mutex::new(HashSet::new())),
            thumbnail_cancellations: Arc::new(Mutex::new(HashMap::new())),
            backup_lock: Arc::new(Mutex::new(())),
            file_operation_lock: Arc::new(Mutex::new(())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

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

    #[tokio::test]
    async fn migrates_a_version_nine_database_to_managed_projects_and_trash() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("volum.sqlite3");
        let options = SqliteConnectOptions::from_str(database_path.to_string_lossy().as_ref())
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let all = sqlx::migrate!("./migrations");
        let through_nine = sqlx::migrate::Migrator {
            migrations: Cow::Owned(
                all.iter()
                    .filter(|migration| migration.version <= 9)
                    .cloned()
                    .collect(),
            ),
            ignore_missing: false,
            locking: true,
            no_tx: false,
        };
        through_nine.run(&pool).await.unwrap();
        pool.close().await;

        let state = AppState::initialize(directory.path()).await.unwrap();

        let versions: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&state.pool)
                .await
                .unwrap();
        let trash_columns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('models') WHERE name IN ('trashed_at', 'trash_original_relative_path')")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(versions.last(), Some(&13));
        assert_eq!(trash_columns, 2);
    }

    #[tokio::test]
    async fn upgrades_every_previous_schema_without_foreign_key_damage() {
        let all = sqlx::migrate!("./migrations");
        let latest = all.iter().map(|migration| migration.version).max().unwrap();

        for starting_version in 1..latest {
            let directory = tempfile::tempdir().unwrap();
            let database_path = directory.path().join("volum.sqlite3");
            let options = SqliteConnectOptions::from_str(database_path.to_string_lossy().as_ref())
                .unwrap()
                .create_if_missing(true)
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .unwrap();
            let partial = sqlx::migrate::Migrator {
                migrations: Cow::Owned(
                    all.iter()
                        .filter(|migration| migration.version <= starting_version)
                        .cloned()
                        .collect(),
                ),
                ignore_missing: false,
                locking: true,
                no_tx: false,
            };
            partial.run(&pool).await.unwrap();
            pool.close().await;

            let state = AppState::initialize(directory.path()).await.unwrap();
            let applied: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
                .fetch_one(&state.pool)
                .await
                .unwrap();
            let foreign_key_errors: Vec<(String, i64, String, i64)> =
                sqlx::query_as("PRAGMA foreign_key_check")
                    .fetch_all(&state.pool)
                    .await
                    .unwrap();

            assert_eq!(applied, latest, "failed from schema v{starting_version}");
            assert!(
                foreign_key_errors.is_empty(),
                "foreign key damage after upgrading schema v{starting_version}: {foreign_key_errors:?}"
            );
            assert!(
                directory
                    .path()
                    .join(format!(
                        "volum.sqlite3.pre-migration-v{starting_version}.bak"
                    ))
                    .is_file(),
                "upgrade from schema v{starting_version} did not preserve a rollback database"
            );
            state.pool.close().await;
        }
    }
}
