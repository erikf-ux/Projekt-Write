//! Rows of the Mesh (section 15, ADR 0022): what a view draws, read in a
//! few whole-table queries, and the stored connections, suggestions and
//! positions. Static SQL only.

use sqlx::{Row, SqliteConnection, SqlitePool};

/// A document or medium outside the trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRow {
    pub id: String,
    /// `document`, or the kind of a medium (`image`, `pdf`, `attachment`).
    pub kind: String,
    pub title: String,
    pub archived: bool,
    /// `true` for a document.
    pub is_document: bool,
    pub import_folder: Option<String>,
}

/// A subnet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetNodeRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub parent_id: Option<String>,
}

/// A membership of an element in a subnet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRow {
    pub element_id: String,
    pub subnet_id: String,
    pub is_primary: bool,
}

/// A confirmed connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub kind: String,
    pub note: Option<String>,
    pub created_at: String,
}

/// A stored position.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionRow {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub pinned: bool,
}

/// Documents and media outside the trash, documents first.
pub async fn elements(pool: &SqlitePool) -> Result<Vec<ElementRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, 'document' AS kind, title, archived_at IS NOT NULL AS archived, 1 AS is_document, \
                NULL AS import_folder \
         FROM documents WHERE deleted_at IS NULL \
         UNION ALL \
         SELECT id, kind, title, archived_at IS NOT NULL, 0, import_folder \
         FROM media WHERE deleted_at IS NULL \
         ORDER BY 5 DESC, 3, 1",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(ElementRow {
                id: row.try_get(0)?,
                kind: row.try_get(1)?,
                title: row.try_get(2)?,
                archived: row.try_get::<i64, _>(3)? != 0,
                is_document: row.try_get::<i64, _>(4)? != 0,
                import_folder: row.try_get(5)?,
            })
        })
        .collect()
}

/// Every subnet, by name.
pub async fn subnets(pool: &SqlitePool) -> Result<Vec<SubnetNodeRow>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, name, color, parent_id FROM subnets ORDER BY name_key, id")
        .fetch_all(pool)
        .await?;
    rows.iter()
        .map(|row| {
            Ok(SubnetNodeRow {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
                color: row.try_get(2)?,
                parent_id: row.try_get(3)?,
            })
        })
        .collect()
}

/// Every membership, oldest first per element.
pub async fn members(pool: &SqlitePool) -> Result<Vec<MemberRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT coalesce(document_id, media_id), subnet_id, is_primary FROM subnet_memberships \
         ORDER BY 1, created_at, subnet_id",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(MemberRow {
                element_id: row.try_get(0)?,
                subnet_id: row.try_get(1)?,
                is_primary: row.try_get::<i64, _>(2)? != 0,
            })
        })
        .collect()
}

/// References: internal links between documents and media in documents,
/// as (from, to). A link to a document that does not exist is left out.
pub async fn references(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT l.source_id, l.target_id FROM document_links AS l \
         JOIN documents AS d ON d.id = l.target_id \
         UNION \
         SELECT document_id, media_id FROM document_media \
         ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
        .collect()
}

fn connection_from(row: &sqlx::sqlite::SqliteRow) -> Result<ConnectionRow, sqlx::Error> {
    Ok(ConnectionRow {
        id: row.try_get(0)?,
        node_a: row.try_get(1)?,
        node_b: row.try_get(2)?,
        kind: row.try_get(3)?,
        note: row.try_get(4)?,
        created_at: row.try_get(5)?,
    })
}

/// Every confirmed connection.
pub async fn connections(pool: &SqlitePool) -> Result<Vec<ConnectionRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, node_a, node_b, kind, note, created_at FROM mesh_connections \
         ORDER BY node_a, node_b, kind",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(connection_from)
    .collect()
}

pub async fn connection(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<ConnectionRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, node_a, node_b, kind, note, created_at FROM mesh_connections WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(connection)
    .await?
    .as_ref()
    .map(connection_from)
    .transpose()
}

pub async fn node_exists(connection: &mut SqliteConnection, id: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM mesh_nodes WHERE id = ?1")
            .bind(id)
            .fetch_one(connection)
            .await?
            > 0,
    )
}

pub async fn connection_between(
    connection: &mut SqliteConnection,
    node_a: &str,
    node_b: &str,
    kind: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM mesh_connections WHERE node_a = ?1 AND node_b = ?2 AND kind = ?3",
    )
    .bind(node_a)
    .bind(node_b)
    .bind(kind)
    .fetch_optional(connection)
    .await
}

pub async fn insert_connection(
    connection: &mut SqliteConnection,
    row: &ConnectionRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mesh_connections (id, node_a, node_b, kind, note, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&row.id)
    .bind(&row.node_a)
    .bind(&row.node_b)
    .bind(&row.kind)
    .bind(&row.note)
    .bind(&row.created_at)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn delete_connection(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM mesh_connections WHERE id = ?1")
        .bind(id)
        .execute(connection)
        .await?
        .rows_affected())
}

/// The id of the layout of a view, if it has one.
pub async fn layout_id(
    pool: &SqlitePool,
    view: &str,
    scope_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM mesh_layouts WHERE view = ?1 AND scope_id = ?2")
        .bind(view)
        .bind(scope_id)
        .fetch_optional(pool)
        .await
}

/// The layout of a view, created if missing.
pub async fn ensure_layout(
    connection: &mut SqliteConnection,
    new_id: &str,
    view: &str,
    scope_id: &str,
    algorithm: Option<&str>,
    now: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query(
        "INSERT INTO mesh_layouts (id, view, scope_id, algorithm, updated_at) \
         VALUES (?1, ?2, ?3, coalesce(?4, ''), ?5) \
         ON CONFLICT (view, scope_id) DO UPDATE SET \
             algorithm = coalesce(?4, mesh_layouts.algorithm), updated_at = ?5",
    )
    .bind(new_id)
    .bind(view)
    .bind(scope_id)
    .bind(algorithm)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    sqlx::query_scalar("SELECT id FROM mesh_layouts WHERE view = ?1 AND scope_id = ?2")
        .bind(view)
        .bind(scope_id)
        .fetch_one(connection)
        .await
}

/// Positions of a layout.
pub async fn positions(
    pool: &SqlitePool,
    layout_id: &str,
) -> Result<Vec<PositionRow>, sqlx::Error> {
    sqlx::query("SELECT node_id, x, y, pinned FROM mesh_positions WHERE layout_id = ?1")
        .bind(layout_id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(|row| {
            Ok(PositionRow {
                node_id: row.try_get(0)?,
                x: row.try_get(1)?,
                y: row.try_get(2)?,
                pinned: row.try_get::<i64, _>(3)? != 0,
            })
        })
        .collect()
}

pub async fn upsert_position(
    connection: &mut SqliteConnection,
    layout_id: &str,
    position: &PositionRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mesh_positions (layout_id, node_id, x, y, pinned) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT (layout_id, node_id) DO UPDATE SET x = ?3, y = ?4, pinned = ?5",
    )
    .bind(layout_id)
    .bind(&position.node_id)
    .bind(position.x)
    .bind(position.y)
    .bind(i64::from(position.pinned))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn delete_position(
    connection: &mut SqliteConnection,
    layout_id: &str,
    node_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM mesh_positions WHERE layout_id = ?1 AND node_id = ?2")
        .bind(layout_id)
        .bind(node_id)
        .execute(connection)
        .await?;
    Ok(())
}

/// Tags of documents and media, as (element, tag).
pub async fn element_tags(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query(
        "SELECT document_id, tag_id FROM document_tags \
         UNION ALL SELECT media_id, tag_id FROM media_tags ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
    .collect()
}

/// The indexed text of each document (search index, ADR 0020).
pub async fn document_texts(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query(
        "SELECT e.document_id, w.body FROM search_entries AS e \
         JOIN search_words AS w ON w.rowid = e.id \
         WHERE e.document_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| {
        Ok((
            row.try_get(0)?,
            row.try_get::<Option<String>, _>(1)?.unwrap_or_default(),
        ))
    })
    .collect()
}

/// Pairs that are connected or were suggested, whatever was decided.
pub async fn known_pairs(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query(
        "SELECT node_a, node_b FROM mesh_connections \
         UNION SELECT node_a, node_b FROM mesh_suggestions",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
    .collect()
}

/// A row of `mesh_suggestions` with the titles of both ends.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionRow {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub score: f64,
    pub reason: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
}

pub async fn insert_suggestion(
    connection: &mut SqliteConnection,
    row: &SuggestionRow,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "INSERT INTO mesh_suggestions (id, node_a, node_b, score, reason, status, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6) ON CONFLICT (node_a, node_b) DO NOTHING",
    )
    .bind(&row.id)
    .bind(&row.node_a)
    .bind(&row.node_b)
    .bind(row.score)
    .bind(&row.reason)
    .bind(&row.created_at)
    .execute(connection)
    .await?
    .rows_affected()
        > 0)
}

fn suggestion_from(row: &sqlx::sqlite::SqliteRow) -> Result<SuggestionRow, sqlx::Error> {
    Ok(SuggestionRow {
        id: row.try_get(0)?,
        node_a: row.try_get(1)?,
        node_b: row.try_get(2)?,
        score: row.try_get(3)?,
        reason: row.try_get(4)?,
        status: row.try_get(5)?,
        created_at: row.try_get(6)?,
        decided_at: row.try_get(7)?,
    })
}

/// Open suggestions, the best first.
pub async fn open_suggestions(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<SuggestionRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, node_a, node_b, score, reason, status, created_at, decided_at \
         FROM mesh_suggestions WHERE status = 'open' ORDER BY score DESC, node_a, node_b LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?
    .iter()
    .map(suggestion_from)
    .collect()
}

/// Every suggestion, for checks and tests.
pub async fn all_suggestions(pool: &SqlitePool) -> Result<Vec<SuggestionRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, node_a, node_b, score, reason, status, created_at, decided_at \
         FROM mesh_suggestions ORDER BY node_a, node_b",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(suggestion_from)
    .collect()
}

pub async fn suggestion(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SuggestionRow>, sqlx::Error> {
    sqlx::query(
        "SELECT id, node_a, node_b, score, reason, status, created_at, decided_at \
         FROM mesh_suggestions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(connection)
    .await?
    .as_ref()
    .map(suggestion_from)
    .transpose()
}

pub async fn decide_suggestion(
    connection: &mut SqliteConnection,
    id: &str,
    status: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE mesh_suggestions SET status = ?2, decided_at = ?3 WHERE id = ?1 AND status = 'open'",
    )
    .bind(id)
    .bind(status)
    .bind(now)
    .execute(connection)
    .await?
    .rows_affected())
}

/// Titles of nodes (documents, media, subnets) by id.
pub async fn node_titles(pool: &SqlitePool) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    sqlx::query(
        "SELECT id, 'document', title FROM documents \
         UNION ALL SELECT id, 'media', title FROM media \
         UNION ALL SELECT id, 'subnet', name FROM subnets",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| Ok((row.try_get(0)?, row.try_get(1)?, row.try_get(2)?)))
    .collect()
}
