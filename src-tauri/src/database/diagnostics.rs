//! Read-only database diagnostics used by the system check.

use std::path::Path;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, SqlitePool};

/// Version of the bundled SQLite library.
pub async fn sqlite_version(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(pool)
        .await
}

/// Compile options of the bundled SQLite library (without the `SQLITE_` prefix).
pub async fn compile_options(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT compile_options FROM pragma_compile_options")
        .fetch_all(pool)
        .await
}

/// Whether foreign key enforcement is on for connections of this pool.
pub async fn foreign_keys_enabled(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let value: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(pool)
        .await?;
    Ok(value == 1)
}

/// `PRAGMA quick_check` of every table that holds rows, one table at a time
/// (with its indexes), without the FTS5 tables themselves. Since SQLite 3.44
/// a check of the whole database also re-tokenises the full text of every
/// FTS5 table (ADR 0020, decision 14); their shadow tables, which hold the
/// pages, are checked here like any other table. Pages on the freelist are
/// only covered by the check of the whole database.
pub async fn quick_check_tables(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema \
         WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    for table in tables {
        let rows: Vec<String> = sqlx::query_scalar("SELECT * FROM pragma_quick_check(?1)")
            .bind(&table)
            .fetch_all(pool)
            .await?;
        if rows.len() != 1 || rows[0] != "ok" {
            log::error!("quick_check of {table}: {}", rows.join("; "));
            return Ok(false);
        }
    }
    Ok(true)
}

/// `PRAGMA quick_check` (section 18: run at start). `true` means `ok`.
pub async fn quick_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_all(pool)
        .await?;
    Ok(rows.len() == 1 && rows[0] == "ok")
}

/// `PRAGMA integrity_check` (section 18: after backup and restore).
pub async fn integrity_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(pool)
        .await?;
    Ok(rows.len() == 1 && rows[0] == "ok")
}

/// `PRAGMA integrity_check` on a database file that is not open elsewhere,
/// through a single read-only connection (backups, section 18).
pub async fn integrity_check_file(path: &Path) -> Result<bool, sqlx::Error> {
    let mut connection = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .connect()
        .await?;
    // Closed in every case before returning: a dropped connection is closed
    // later by its worker thread, and until then Windows refuses to delete
    // the file (a damaged database fails the pragma with an error).
    let rows: Result<Vec<String>, sqlx::Error> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&mut connection)
        .await;
    let closed = connection.close().await;
    let rows = rows?;
    closed?;
    Ok(rows.len() == 1 && rows[0] == "ok")
}

/// Current journal mode, lower case (for example `wal`, `delete`, `memory`).
pub async fn journal_mode(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
}

/// Current `synchronous` level (2 = FULL).
pub async fn synchronous(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(pool)
        .await
}

/// Current busy timeout in milliseconds.
pub async fn busy_timeout_ms(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(pool)
        .await
}
