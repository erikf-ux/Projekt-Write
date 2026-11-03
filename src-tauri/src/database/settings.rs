//! SQL for `application_settings` (migration 0001). Values are JSON texts;
//! SQLite checks `json_valid` on every write.

use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM application_settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO application_settings (key, value, updated_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove(pool: &SqlitePool, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM application_settings WHERE key = ?1")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}
