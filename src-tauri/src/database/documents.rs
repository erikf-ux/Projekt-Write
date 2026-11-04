//! SQL for the `documents` and `document_links` tables (migrations 0002 to
//! 0005). Only the document repository (`services::documents`) calls these
//! functions.
//!
//! Content changes write the row and the links of the document in one
//! transaction (ADR 0015, decision 14): the links never describe another
//! content than the row.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

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
    /// Favourite since (migration 0004).
    pub favorited_at: Option<String>,
    /// Last opened in a tab (migration 0004).
    pub last_opened_at: Option<String>,
}

macro_rules! select_documents {
    ($tail:literal) => {
        concat!(
            "SELECT id, title, format, size_bytes, content_sha256, created_at, updated_at, archived_at, deleted_at, original_name, favorited_at, last_opened_at FROM documents ",
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
        favorited_at: row.try_get("favorited_at")?,
        last_opened_at: row.try_get("last_opened_at")?,
    })
}

/// Inserts a new document and its links.
pub async fn insert(
    pool: &SqlitePool,
    row: &DocumentRow,
    links: &[String],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    insert_row(&mut transaction, row).await?;
    replace_links(&mut transaction, &row.id, links).await?;
    transaction.commit().await
}

async fn insert_row(
    connection: &mut SqliteConnection,
    row: &DocumentRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO documents (id, title, format, size_bytes, content_sha256, created_at, updated_at, archived_at, deleted_at, original_name, favorited_at, last_opened_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
    .bind(&row.favorited_at)
    .bind(&row.last_opened_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

/// Replaces the outgoing links of `source`.
async fn replace_links(
    connection: &mut SqliteConnection,
    source: &str,
    targets: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM document_links WHERE source_id = ?1")
        .bind(source)
        .execute(&mut *connection)
        .await?;
    for target in targets.iter().filter(|target| target.as_str() != source) {
        sqlx::query("INSERT OR IGNORE INTO document_links (source_id, target_id) VALUES (?1, ?2)")
            .bind(source)
            .bind(target)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Documents not in the trash that link to `target`, by title.
pub async fn backlinks(pool: &SqlitePool, target: &str) -> Result<Vec<DocumentRow>, sqlx::Error> {
    sqlx::query(select_documents!(
        "WHERE id IN (SELECT source_id FROM document_links WHERE target_id = ?1) \
         AND deleted_at IS NULL ORDER BY title, id"
    ))
    .bind(target)
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

/// The link targets of `source` (for tests and checks).
pub async fn links_of(pool: &SqlitePool, source: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT target_id FROM document_links WHERE source_id = ?1 ORDER BY target_id",
    )
    .bind(source)
    .fetch_all(pool)
    .await
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
/// New content of a document: size, checksum, time and links.
pub async fn update_content(
    pool: &SqlitePool,
    id: &str,
    size_bytes: i64,
    content_sha256: &str,
    updated_at: &str,
    links: &[String],
) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE documents SET size_bytes = ?2, content_sha256 = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(id)
    .bind(size_bytes)
    .bind(content_sha256)
    .bind(updated_at)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if changed > 0 {
        replace_links(&mut transaction, id, links).await?;
    }
    transaction.commit().await?;
    Ok(changed)
}

/// Converts a document (ADR 0015, decision 13): new format, content and
/// links.
pub async fn update_format(
    pool: &SqlitePool,
    id: &str,
    format: &str,
    size_bytes: i64,
    content_sha256: &str,
    updated_at: &str,
    links: &[String],
) -> Result<u64, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE documents SET format = ?2, size_bytes = ?3, content_sha256 = ?4, updated_at = ?5 \
         WHERE id = ?1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(format)
    .bind(size_bytes)
    .bind(content_sha256)
    .bind(updated_at)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if changed > 0 {
        replace_links(&mut transaction, id, links).await?;
    }
    transaction.commit().await?;
    Ok(changed)
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

/// Marks a document as a favourite (`Some`) or not (`None`). Returns the number
/// of changed rows.
pub async fn set_favorited(
    pool: &SqlitePool,
    id: &str,
    favorited_at: Option<&str>,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE documents SET favorited_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(favorited_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

/// Records that a document was opened ("Zuletzt verwendet").
pub async fn set_last_opened(
    pool: &SqlitePool,
    id: &str,
    last_opened_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE documents SET last_opened_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(last_opened_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
