//! SQLite access through SQLx (section 18).
//!
//! * One writer connection, a pool of read-only connections (section 8).
//! * `foreign_keys=ON`, `synchronous=FULL`, `busy_timeout` on every connection.
//! * WAL on local disks, rollback journal (`DELETE`) for synced or network
//!   folders (section 9). Choosing the mode is the caller's job (Phase 2).
//! * Forward-only migrations embedded at compile time from `migrations/`.
//!
//! Only this module and the repositories built on it touch SQL.

pub mod diagnostics;
pub mod documents;
pub mod fts;
pub mod history;
pub mod settings;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

/// All schema migrations, embedded at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const READER_CONNECTIONS: u32 = 4;

/// Journal mode for file databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    /// Write-ahead log. Default for local disks.
    Wal,
    /// Rollback journal (`journal_mode=DELETE`) for sync folders and network drives.
    Rollback,
}

impl JournalMode {
    fn to_sqlx(self) -> SqliteJournalMode {
        match self {
            JournalMode::Wal => SqliteJournalMode::Wal,
            JournalMode::Rollback => SqliteJournalMode::Delete,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    /// The database contains migrations this build does not know.
    #[error("database schema version {found} is newer than supported version {supported}")]
    SchemaTooNew { found: i64, supported: i64 },
    #[error("migration failed: {0}")]
    Migration(#[from] MigrateError),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// Result of running the migrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions applied by this run, in order. Empty if the schema was current.
    pub applied: Vec<i64>,
    /// Schema version after the run.
    pub version: i64,
}

/// Handle to one SQLite database.
#[derive(Debug, Clone)]
pub struct Database {
    writer: SqlitePool,
    reader: SqlitePool,
}

impl Database {
    /// Opens (and creates if missing) a database file.
    ///
    /// Migrations are not run automatically: the library creates a backup
    /// before migrating, so the caller decides when to call
    /// [`Database::migrate`].
    pub async fn open_file(path: &Path, journal: JournalMode) -> Result<Self, DatabaseError> {
        Self::open_with(path, journal, true).await
    }

    /// Opens an existing database file; fails if it does not exist.
    pub async fn open_existing(path: &Path, journal: JournalMode) -> Result<Self, DatabaseError> {
        Self::open_with(path, journal, false).await
    }

    async fn open_with(
        path: &Path,
        journal: JournalMode,
        create: bool,
    ) -> Result<Self, DatabaseError> {
        let base = SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(BUSY_TIMEOUT)
            .journal_mode(journal.to_sqlx());

        // The writer is opened first: it creates the file and, in WAL mode,
        // the -wal/-shm files the read-only connections need.
        let writer = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .connect_with(base.clone().create_if_missing(create))
            .await?;
        let reader = SqlitePoolOptions::new()
            .max_connections(READER_CONNECTIONS)
            .connect_with(base.read_only(true))
            .await?;
        Ok(Self { writer, reader })
    }

    /// Opens a private in-memory database.
    ///
    /// Used while no library is open (Phase 1, see ADR 0004) and in tests.
    /// A single connection serves reads and writes and is never recycled,
    /// because closing it would discard the database.
    pub async fn open_in_memory() -> Result<Self, DatabaseError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(BUSY_TIMEOUT)
            .journal_mode(SqliteJournalMode::Memory);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        Ok(Self {
            writer: pool.clone(),
            reader: pool,
        })
    }

    /// The single writer connection. All writes go through it.
    pub fn writer(&self) -> &SqlitePool {
        &self.writer
    }

    /// Read-only connections.
    pub fn reader(&self) -> &SqlitePool {
        &self.reader
    }

    /// Highest schema version this build knows.
    pub fn supported_version() -> i64 {
        MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
    }

    /// Runs pending migrations. Each migration runs in its own transaction.
    ///
    /// Rejects databases written by a newer version of the application
    /// before changing anything.
    pub async fn migrate(&self) -> Result<MigrationReport, DatabaseError> {
        let supported = Self::supported_version();
        let before = self.applied_versions().await?;
        if let Some(&found) = before.iter().max()
            && found > supported
        {
            return Err(DatabaseError::SchemaTooNew { found, supported });
        }

        MIGRATOR
            .run(&self.writer)
            .await
            .map_err(|error| match error {
                MigrateError::VersionMissing(found) => {
                    DatabaseError::SchemaTooNew { found, supported }
                }
                other => DatabaseError::Migration(other),
            })?;

        let after = self.applied_versions().await?;
        let applied = after
            .iter()
            .copied()
            .filter(|v| !before.contains(v))
            .collect();
        let version = after.iter().copied().max().unwrap_or(0);
        Ok(MigrationReport { applied, version })
    }

    /// Current schema version (0 for an empty database).
    pub async fn schema_version(&self) -> Result<i64, DatabaseError> {
        Ok(self
            .applied_versions()
            .await?
            .into_iter()
            .max()
            .unwrap_or(0))
    }

    async fn applied_versions(&self) -> Result<Vec<i64>, DatabaseError> {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = '_sqlx_migrations')",
        )
        .fetch_one(&self.writer)
        .await?;
        if !table_exists {
            return Ok(Vec::new());
        }
        let rows =
            sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = 1 ORDER BY version")
                .fetch_all(&self.writer)
                .await?;
        Ok(rows.iter().map(|row| row.get::<i64, _>(0)).collect())
    }

    /// Writes a consistent, compacted copy of the database to `target`
    /// (`VACUUM INTO`, section 21). `target` must not exist. A copy that was
    /// interrupted may be incomplete; callers check it with
    /// [`diagnostics::integrity_check`] before relying on it.
    pub async fn vacuum_into(&self, target: &Path) -> Result<(), DatabaseError> {
        let target = target.to_str().ok_or_else(|| {
            DatabaseError::Sqlx(sqlx::Error::Configuration(
                "backup path is not valid UTF-8".into(),
            ))
        })?;
        sqlx::query("VACUUM INTO ?1")
            .bind(target)
            .execute(&self.writer)
            .await?;
        Ok(())
    }

    /// Runs the migrations up to and including `version` (tests of the
    /// upgrade path from every earlier schema version).
    pub async fn migrate_to(&self, version: i64) -> Result<(), DatabaseError> {
        MIGRATOR.run_to(version, &self.writer).await?;
        Ok(())
    }

    /// Closes all connections and waits until they are released.
    pub async fn close(&self) {
        self.reader.close().await;
        self.writer.close().await;
    }
}
