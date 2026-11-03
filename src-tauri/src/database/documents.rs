//! SQL for the `documents` table (migrations 0002 and 0003). Only the
//! document repository (`services::documents`) calls these functions.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

/// One row of `documents`, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub id: String,
    pub title: String,
    pub format: String,
    pub size_bytes: i64,
    pub content_sha256: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    /// File name at import (migration 0003).
    pub original_name: Option<String>,
}

macro_rules! select_documents {
    ($tail:literal) => {
        concat!(
            "SELECT id, title, format, size_bytes, content_sha256, created_at, updated_at, archived_at, deleted_at, original_name FROM documents ",
            $tail
        )
    };
}

fn from_row(row: &SqliteRow) -> Result<DocumentRow, sqlx::Error> {
    Ok(DocumentRow {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        format: row.try_get("format")?,
        size_bytes: row.try_get("size_bytes")?,
        content_sha256: row.try_get("content_sha256")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        archived_at: row.try_get("archived_at")?,
        deleted_at: row.try_get("deleted_at")?,
        original_name: row.try_get("original_name")?,
    })
}

pub async fn insert(pool: &SqlitePool, row: &DocumentRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO documents (id, title, format, size_bytes, content_sha256, created_at, updated_at, archived_at, deleted_at, original_name) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&row.id)
    .bind(&row.title)
    .bind(&row.format)
    .bind(row.size_bytes)
    .bind(&row.content_sha256)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .bind(&row.archived_at)
    .bind(&row.deleted_at)
    .bind(&row.original_name)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<DocumentRow>, sqlx::Error> {
    sqlx::query(select_documents!("WHERE id = ?1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(from_row)
        .transpose()
}

/// Documents not in the trash, most recently changed first.
pub async fn list_active(pool: &SqlitePool) -> Result<Vec<DocumentRow>, sqlx::Error> {
    sqlx::query(select_documents!(
        "WHERE deleted_at IS NULL ORDER BY updated_at DESC, id"
    ))
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

/// A document not in the trash with exactly this content (duplicate check
/// of the import, section 9).
pub async fn find_active_by_sha256(
    pool: &SqlitePool,
    content_sha256: &str,
) -> Result<Option<DocumentRow>, sqlx::Error> {
    sqlx::query(select_documents!(
        "WHERE content_sha256 = ?1 AND deleted_at IS NULL ORDER BY created_at, id LIMIT 1"
    ))
    .bind(content_sha256)
    .fetch_optional(pool)
    .await?
    .as_ref()
    .map(from_row)
    .transpose()
}

pub async fn count_active(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM documents WHERE deleted_at IS NULL")
        .fetch_one(pool)
        .await
}

/// `(id, format)` of every row, including documents in the trash (their
/// files stay until the trash is emptied).
pub async fn all_file_keys(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query("SELECT id, format FROM documents")
        .fetch_all(pool)
        .await?
        .iter()
        .map(|row| Ok((row.try_get("id")?, row.try_get("format")?)))
        .collect()
}

/// Records new content metadata. Returns the number of changed rows.
pub async fn update_content(
    pool: &SqlitePool,
    id: &str,
    size_bytes: i64,
    content_sha256: &str,
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE documents SET size_bytes = ?2, content_sha256 = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(id)
    .bind(size_bytes)
    .bind(content_sha256)
    .bind(updated_at)
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn update_title(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE documents SET title = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(title)
            .bind(updated_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

/// Moves a document to the trash (`deleted_at`, section 18). The row and the
/// file stay.
pub async fn set_deleted(
    pool: &SqlitePool,
    id: &str,
    deleted_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE documents SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
            .bind(id)
            .bind(deleted_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
