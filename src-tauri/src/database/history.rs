//! SQL for the `document_history` table (migration 0006). Only the history
//! service (`services::history`) calls these functions.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

/// A version without its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRow {
    pub id: String,
    pub document_id: String,
    pub kind: String,
    pub name: Option<String>,
    pub format: String,
    pub size_bytes: i64,
    pub content_sha256: String,
    pub created_at: String,
}

macro_rules! select_versions {
    ($extra:literal, $tail:literal) => {
        concat!(
            "SELECT id, document_id, kind, name, format, size_bytes, content_sha256, created_at",
            $extra,
            " FROM document_history ",
            $tail
        )
    };
}

fn from_row(row: &SqliteRow) -> Result<VersionRow, sqlx::Error> {
    Ok(VersionRow {
        id: row.try_get("id")?,
        document_id: row.try_get("document_id")?,
        kind: row.try_get("kind")?,
        name: row.try_get("name")?,
        format: row.try_get("format")?,
        size_bytes: row.try_get("size_bytes")?,
        content_sha256: row.try_get("content_sha256")?,
        created_at: row.try_get("created_at")?,
    })
}

pub async fn insert(
    pool: &SqlitePool,
    row: &VersionRow,
    content: &[u8],
    compression: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO document_history (id, document_id, kind, name, format, content, compression, size_bytes, content_sha256, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&row.id)
    .bind(&row.document_id)
    .bind(&row.kind)
    .bind(&row.name)
    .bind(&row.format)
    .bind(content)
    .bind(compression)
    .bind(row.size_bytes)
    .bind(&row.content_sha256)
    .bind(&row.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// The versions of a document, newest first.
pub async fn list(pool: &SqlitePool, document_id: &str) -> Result<Vec<VersionRow>, sqlx::Error> {
    sqlx::query(select_versions!(
        "",
        "WHERE document_id = ?1 ORDER BY created_at DESC, id DESC"
    ))
    .bind(document_id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

/// The newest version of a document.
pub async fn latest(
    pool: &SqlitePool,
    document_id: &str,
) -> Result<Option<VersionRow>, sqlx::Error> {
    sqlx::query(select_versions!(
        "",
        "WHERE document_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1"
    ))
    .bind(document_id)
    .fetch_optional(pool)
    .await?
    .as_ref()
    .map(from_row)
    .transpose()
}

/// A version with its stored (compressed) content and the compression.
pub async fn get(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<(VersionRow, Vec<u8>, String)>, sqlx::Error> {
    let Some(row) = sqlx::query(select_versions!(", content, compression", "WHERE id = ?1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some((
        from_row(&row)?,
        row.try_get("content")?,
        row.try_get("compression")?,
    )))
}

/// Turns a version into a named one.
pub async fn set_name(pool: &SqlitePool, id: &str, name: &str) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE document_history SET kind = 'named', name = ?2 WHERE id = ?1")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub async fn delete(pool: &SqlitePool, ids: &[String]) -> Result<u64, sqlx::Error> {
    let mut removed = 0;
    for id in ids {
        removed += sqlx::query("DELETE FROM document_history WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected();
    }
    Ok(removed)
}

/// Documents that have versions.
pub async fn documents_with_versions(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT DISTINCT document_id FROM document_history ORDER BY document_id")
        .fetch_all(pool)
        .await
}
