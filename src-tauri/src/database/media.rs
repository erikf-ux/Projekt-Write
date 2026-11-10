//! SQL for the `media` and `document_media` tables (migration 0008; ADR
//! 0019). Only the media repository (`services::media`) and the document
//! repository (for `document_media` on save) call these functions.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

/// One row of `media`, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRow {
    pub id: String,
    /// `image`, `pdf` or `attachment`.
    pub kind: String,
    pub mime_type: String,
    pub extension: String,
    pub title: String,
    pub description: String,
    pub original_name: String,
    pub import_folder: Option<String>,
    pub size_bytes: i64,
    pub sha256: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub page_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub favorited_at: Option<String>,
    pub last_opened_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
}

macro_rules! select_media {
    ($tail:literal) => {
        concat!(
            "SELECT id, kind, mime_type, extension, title, description, original_name, import_folder, \
                    size_bytes, sha256, width, height, page_count, created_at, updated_at, \
                    favorited_at, last_opened_at, archived_at, deleted_at FROM media ",
            $tail
        )
    };
}

fn from_row(row: &SqliteRow) -> Result<MediaRow, sqlx::Error> {
    Ok(MediaRow {
        id: row.try_get("id")?,
        kind: row.try_get("kind")?,
        mime_type: row.try_get("mime_type")?,
        extension: row.try_get("extension")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        original_name: row.try_get("original_name")?,
        import_folder: row.try_get("import_folder")?,
        size_bytes: row.try_get("size_bytes")?,
        sha256: row.try_get("sha256")?,
        width: row.try_get("width")?,
        height: row.try_get("height")?,
        page_count: row.try_get("page_count")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        favorited_at: row.try_get("favorited_at")?,
        last_opened_at: row.try_get("last_opened_at")?,
        archived_at: row.try_get("archived_at")?,
        deleted_at: row.try_get("deleted_at")?,
    })
}

pub async fn insert(connection: &mut SqliteConnection, row: &MediaRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO media (id, kind, mime_type, extension, title, description, original_name, import_folder, \
                            size_bytes, sha256, width, height, page_count, created_at, updated_at, \
                            favorited_at, last_opened_at, archived_at, deleted_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
    )
    .bind(&row.id)
    .bind(&row.kind)
    .bind(&row.mime_type)
    .bind(&row.extension)
    .bind(&row.title)
    .bind(&row.description)
    .bind(&row.original_name)
    .bind(&row.import_folder)
    .bind(row.size_bytes)
    .bind(&row.sha256)
    .bind(row.width)
    .bind(row.height)
    .bind(row.page_count)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .bind(&row.favorited_at)
    .bind(&row.last_opened_at)
    .bind(&row.archived_at)
    .bind(&row.deleted_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<MediaRow>, sqlx::Error> {
    sqlx::query(select_media!("WHERE id = ?1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(from_row)
        .transpose()
}

/// Media outside the trash, newest first.
pub async fn list_active(pool: &SqlitePool) -> Result<Vec<MediaRow>, sqlx::Error> {
    sqlx::query(select_media!(
        "WHERE deleted_at IS NULL ORDER BY created_at DESC, id"
    ))
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

/// Media in the trash, last deleted first.
pub async fn list_trash(pool: &SqlitePool) -> Result<Vec<MediaRow>, sqlx::Error> {
    sqlx::query(select_media!(
        "WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC, id"
    ))
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}

/// The oldest medium outside the trash with this checksum.
pub async fn find_active_by_sha256(
    connection: &mut SqliteConnection,
    sha256: &str,
) -> Result<Option<MediaRow>, sqlx::Error> {
    sqlx::query(select_media!(
        "WHERE sha256 = ?1 AND deleted_at IS NULL ORDER BY created_at, id LIMIT 1"
    ))
    .bind(sha256)
    .fetch_optional(&mut *connection)
    .await?
    .as_ref()
    .map(from_row)
    .transpose()
}

/// Id, kind and extension of every row (to compare the folders with).
pub async fn all_file_keys(
    pool: &SqlitePool,
) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    sqlx::query_as("SELECT id, kind, extension FROM media ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn ids_of(pool: &SqlitePool, ids: &[String]) -> Result<Vec<String>, sqlx::Error> {
    let mut found = Vec::new();
    for id in ids {
        if let Some(id) = sqlx::query_scalar::<_, String>("SELECT id FROM media WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?
        {
            found.push(id);
        }
    }
    Ok(found)
}

pub async fn update_details(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    description: &str,
    updated_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE media SET title = ?2, description = ?3, updated_at = ?4 WHERE id = ?1")
            .bind(id)
            .bind(title)
            .bind(description)
            .bind(updated_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub async fn set_page_count(pool: &SqlitePool, id: &str, pages: i64) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE media SET page_count = ?2 WHERE id = ?1 AND kind = 'pdf'")
            .bind(id)
            .bind(pages)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub async fn set_favorited(
    pool: &SqlitePool,
    id: &str,
    favorited_at: Option<&str>,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE media SET favorited_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(favorited_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub async fn set_last_opened(
    pool: &SqlitePool,
    id: &str,
    opened_at: &str,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE media SET last_opened_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(opened_at)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

/// Ids of media in the trash since before `cutoff`.
pub async fn trashed_before(pool: &SqlitePool, cutoff: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM media WHERE deleted_at IS NOT NULL AND deleted_at < ?1 ORDER BY id",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
}

/// Removes a medium from the trash for good, if no document uses it any
/// more. Returns the number of rows removed (0: not in the trash or used).
pub async fn delete_trashed_unused(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM media WHERE id = ?1 AND deleted_at IS NOT NULL \
         AND NOT EXISTS (SELECT 1 FROM document_media WHERE media_id = ?1)",
    )
    .bind(id)
    .execute(&mut *connection)
    .await?
    .rows_affected())
}

// --- document_media ----------------------------------------------------------

/// Replaces the media a document uses. Ids of media that do not exist are
/// left out (a `.pwrite` from elsewhere may name media of another library).
pub async fn replace_document_media(
    connection: &mut SqliteConnection,
    document_id: &str,
    uses: &[(String, u32)],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM document_media WHERE document_id = ?1")
        .bind(document_id)
        .execute(&mut *connection)
        .await?;
    for (media_id, page) in uses {
        sqlx::query(
            "INSERT OR IGNORE INTO document_media (document_id, media_id, page) \
             SELECT ?1, id, ?3 FROM media WHERE id = ?2",
        )
        .bind(document_id)
        .bind(media_id)
        .bind(i64::from(*page))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Documents that use a medium, with the pages they refer to.
pub async fn users_of(
    pool: &SqlitePool,
    media_id: &str,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT document_id, page FROM document_media WHERE media_id = ?1 ORDER BY document_id, page",
    )
    .bind(media_id)
    .fetch_all(pool)
    .await
}

/// Number of documents that use a medium.
pub async fn user_count(pool: &SqlitePool, media_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(DISTINCT document_id) FROM document_media WHERE media_id = ?1")
        .bind(media_id)
        .fetch_one(pool)
        .await
}

/// Media a document uses (for the `.pwrite` envelope).
pub async fn media_of_document(
    pool: &SqlitePool,
    document_id: &str,
) -> Result<Vec<MediaRow>, sqlx::Error> {
    sqlx::query(select_media!(
        "WHERE id IN (SELECT media_id FROM document_media WHERE document_id = ?1) ORDER BY id"
    ))
    .bind(document_id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(from_row)
    .collect()
}
