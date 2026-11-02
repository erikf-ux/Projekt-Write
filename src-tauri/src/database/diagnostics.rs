//! Read-only database diagnostics used by the system check.

use sqlx::SqlitePool;

/// Version of the bundled SQLite library.
pub async fn sqlite_version(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT sqlite_version()").fetch_one(pool).await
}

/// Compile options of the bundled SQLite library (without the `SQLITE_` prefix).
pub async fn compile_options(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT compile_options FROM pragma_compile_options")
        .fetch_all(pool)
        .await
}

/// Whether foreign key enforcement is on for connections of this pool.
pub async fn foreign_keys_enabled(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let value: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(pool).await?;
    Ok(value == 1)
}

/// `PRAGMA quick_check` (section 18: run at start). `true` means `ok`.
pub async fn quick_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA quick_check").fetch_all(pool).await?;
    Ok(rows.len() == 1 && rows[0] == "ok")
}

/// `PRAGMA integrity_check` (section 18: after backup and restore).
pub async fn integrity_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check").fetch_all(pool).await?;
    Ok(rows.len() == 1 && rows[0] == "ok")
}

/// Current journal mode, lower case (for example `wal`, `delete`, `memory`).
pub async fn journal_mode(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("PRAGMA journal_mode").fetch_one(pool).await
}

/// Current `synchronous` level (2 = FULL).
pub async fn synchronous(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA synchronous").fetch_one(pool).await
}

/// Current busy timeout in milliseconds.
pub async fn busy_timeout_ms(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA busy_timeout").fetch_one(pool).await
}
