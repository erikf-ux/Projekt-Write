//! SQL for subnets, memberships and tags (migration 0007; ADR 0017). Only
//! the structure service (`services::structure`) calls these functions.
//! Writes take a connection, so the service runs them in one transaction
//! with the snapshots of its undo journal.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

/// One row of `subnets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetRow {
    pub id: String,
    pub name: String,
    pub name_key: String,
    pub description: String,
    pub color: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// One row of `subnet_memberships`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipRow {
    pub subnet_id: String,
    pub document_id: String,
    pub is_primary: bool,
    pub created_at: String,
}

/// One row of `tags`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub name_key: String,
    pub created_at: String,
    pub updated_at: String,
}

/// One row of `document_tags`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTagRow {
    pub document_id: String,
    pub tag_id: String,
    pub created_at: String,
}

/// Trash and archive state of a document (the part of `documents` the
/// structural undo restores).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentState {
    pub deleted_at: Option<String>,
    pub archived_at: Option<String>,
}

macro_rules! select_subnets {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, name_key, description, color, parent_id, created_at, updated_at FROM subnets ",
            $tail
        )
    };
}

macro_rules! select_memberships {
    ($tail:literal) => {
        concat!(
            "SELECT subnet_id, document_id, is_primary, created_at FROM subnet_memberships ",
            $tail
        )
    };
}

macro_rules! select_tags {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, name_key, created_at, updated_at FROM tags ",
            $tail
        )
    };
}

fn subnet_from(row: &SqliteRow) -> Result<SubnetRow, sqlx::Error> {
    Ok(SubnetRow {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        name_key: row.try_get("name_key")?,
        description: row.try_get("description")?,
        color: row.try_get("color")?,
        parent_id: row.try_get("parent_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn membership_from(row: &SqliteRow) -> Result<MembershipRow, sqlx::Error> {
    Ok(MembershipRow {
        subnet_id: row.try_get("subnet_id")?,
        document_id: row.try_get("document_id")?,
        is_primary: row.try_get::<i64, _>("is_primary")? == 1,
        created_at: row.try_get("created_at")?,
    })
}

fn tag_from(row: &SqliteRow) -> Result<TagRow, sqlx::Error> {
    Ok(TagRow {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        name_key: row.try_get("name_key")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn document_tag_from(row: &SqliteRow) -> Result<DocumentTagRow, sqlx::Error> {
    Ok(DocumentTagRow {
        document_id: row.try_get("document_id")?,
        tag_id: row.try_get("tag_id")?,
        created_at: row.try_get("created_at")?,
    })
}

// --- Subnets ---------------------------------------------------------------

pub async fn list_subnets(pool: &SqlitePool) -> Result<Vec<SubnetRow>, sqlx::Error> {
    sqlx::query(select_subnets!("ORDER BY name_key, id"))
        .fetch_all(pool)
        .await?
        .iter()
        .map(subnet_from)
        .collect()
}

pub async fn get_subnet(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<SubnetRow>, sqlx::Error> {
    sqlx::query(select_subnets!("WHERE id = ?1"))
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?
        .as_ref()
        .map(subnet_from)
        .transpose()
}

/// The subnet with this name among the children of `parent` (root if `None`).
pub async fn sibling_named(
    connection: &mut SqliteConnection,
    parent: Option<&str>,
    name_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM subnets WHERE ifnull(parent_id, '') = ifnull(?1, '') AND name_key = ?2",
    )
    .bind(parent)
    .bind(name_key)
    .fetch_optional(&mut *connection)
    .await
}

pub async fn children(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM subnets WHERE parent_id = ?1 ORDER BY name_key, id")
        .bind(id)
        .fetch_all(&mut *connection)
        .await
}

/// All descendants of `id` (not `id` itself), deepest first.
pub async fn descendants(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "WITH RECURSIVE below (id, depth) AS ( \
             SELECT id, 1 FROM subnets WHERE parent_id = ?1 \
             UNION ALL \
             SELECT s.id, b.depth + 1 FROM subnets AS s JOIN below AS b ON s.parent_id = b.id \
         ) \
         SELECT id FROM below ORDER BY depth DESC, id",
    )
    .bind(id)
    .fetch_all(&mut *connection)
    .await
}

/// Number of root subnets (for the next cluster colour).
pub async fn count_roots(connection: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM subnets WHERE parent_id IS NULL")
        .fetch_one(&mut *connection)
        .await
}

pub async fn insert_subnet(
    connection: &mut SqliteConnection,
    row: &SubnetRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO subnets (id, name, name_key, description, color, parent_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.name_key)
    .bind(&row.description)
    .bind(&row.color)
    .bind(&row.parent_id)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

/// Writes every column of an existing subnet.
pub async fn update_subnet(
    connection: &mut SqliteConnection,
    row: &SubnetRow,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE subnets SET name = ?2, name_key = ?3, description = ?4, color = ?5, \
                parent_id = ?6, created_at = ?7, updated_at = ?8 \
         WHERE id = ?1",
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.name_key)
    .bind(&row.description)
    .bind(&row.color)
    .bind(&row.parent_id)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(&mut *connection)
    .await?
    .rows_affected())
}

/// Takes a subnet out of the tree for a moment: no parent and a name key no
/// other row can have (used while the undo journal writes a whole state).
pub async fn detach_subnet(connection: &mut SqliteConnection, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE subnets SET parent_id = NULL, name_key = name_key || char(31) || id WHERE id = ?1",
    )
    .bind(id)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn delete_subnet(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM subnets WHERE id = ?1")
        .bind(id)
        .execute(&mut *connection)
        .await?
        .rows_affected())
}

// --- Memberships -------------------------------------------------------------

pub async fn all_memberships(pool: &SqlitePool) -> Result<Vec<MembershipRow>, sqlx::Error> {
    sqlx::query(select_memberships!(
        "ORDER BY document_id, created_at, subnet_id"
    ))
    .fetch_all(pool)
    .await?
    .iter()
    .map(membership_from)
    .collect()
}

/// Memberships of a document, oldest first.
pub async fn memberships_of(
    connection: &mut SqliteConnection,
    document_id: &str,
) -> Result<Vec<MembershipRow>, sqlx::Error> {
    sqlx::query(select_memberships!(
        "WHERE document_id = ?1 ORDER BY created_at, subnet_id"
    ))
    .bind(document_id)
    .fetch_all(&mut *connection)
    .await?
    .iter()
    .map(membership_from)
    .collect()
}

/// Documents with a membership in this subnet.
pub async fn members_of(
    connection: &mut SqliteConnection,
    subnet_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT document_id FROM subnet_memberships WHERE subnet_id = ?1 ORDER BY document_id",
    )
    .bind(subnet_id)
    .fetch_all(&mut *connection)
    .await
}

/// Replaces the memberships of a document.
pub async fn replace_memberships(
    connection: &mut SqliteConnection,
    document_id: &str,
    rows: &[MembershipRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM subnet_memberships WHERE document_id = ?1")
        .bind(document_id)
        .execute(&mut *connection)
        .await?;
    for row in rows {
        sqlx::query(
            "INSERT INTO subnet_memberships (subnet_id, document_id, is_primary, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&row.subnet_id)
        .bind(document_id)
        .bind(i64::from(row.is_primary))
        .bind(&row.created_at)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

// --- Tags ----------------------------------------------------------------------

pub async fn list_tags(pool: &SqlitePool) -> Result<Vec<TagRow>, sqlx::Error> {
    sqlx::query(select_tags!("ORDER BY name_key, id"))
        .fetch_all(pool)
        .await?
        .iter()
        .map(tag_from)
        .collect()
}

pub async fn get_tag(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<TagRow>, sqlx::Error> {
    sqlx::query(select_tags!("WHERE id = ?1"))
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?
        .as_ref()
        .map(tag_from)
        .transpose()
}

pub async fn tag_by_key(
    connection: &mut SqliteConnection,
    name_key: &str,
) -> Result<Option<TagRow>, sqlx::Error> {
    sqlx::query(select_tags!("WHERE name_key = ?1"))
        .bind(name_key)
        .fetch_optional(&mut *connection)
        .await?
        .as_ref()
        .map(tag_from)
        .transpose()
}

pub async fn insert_tag(
    connection: &mut SqliteConnection,
    row: &TagRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tags (id, name, name_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.name_key)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn update_tag(
    connection: &mut SqliteConnection,
    row: &TagRow,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE tags SET name = ?2, name_key = ?3, created_at = ?4, updated_at = ?5 WHERE id = ?1",
    )
    .bind(&row.id)
    .bind(&row.name)
    .bind(&row.name_key)
    .bind(&row.created_at)
    .bind(&row.updated_at)
    .execute(&mut *connection)
    .await?
    .rows_affected())
}

/// Gives a tag a name key no other row can have (see [`detach_subnet`]).
pub async fn detach_tag(connection: &mut SqliteConnection, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tags SET name_key = name_key || char(31) || id WHERE id = ?1")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    Ok(())
}

pub async fn delete_tag(connection: &mut SqliteConnection, id: &str) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM tags WHERE id = ?1")
        .bind(id)
        .execute(&mut *connection)
        .await?
        .rows_affected())
}

pub async fn all_document_tags(pool: &SqlitePool) -> Result<Vec<DocumentTagRow>, sqlx::Error> {
    sqlx::query(
        "SELECT document_id, tag_id, created_at FROM document_tags ORDER BY document_id, created_at, tag_id",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(document_tag_from)
    .collect()
}

/// Tags of a document, in the order they were given.
pub async fn tags_of(
    connection: &mut SqliteConnection,
    document_id: &str,
) -> Result<Vec<DocumentTagRow>, sqlx::Error> {
    sqlx::query(
        "SELECT document_id, tag_id, created_at FROM document_tags \
         WHERE document_id = ?1 ORDER BY created_at, tag_id",
    )
    .bind(document_id)
    .fetch_all(&mut *connection)
    .await?
    .iter()
    .map(document_tag_from)
    .collect()
}

/// Names of the tags of a document, by name (for the `.pwrite` envelope).
pub async fn tag_names_of(
    pool: &SqlitePool,
    document_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT t.name FROM document_tags AS d JOIN tags AS t ON t.id = d.tag_id \
         WHERE d.document_id = ?1 ORDER BY t.name_key, t.id",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await
}

/// Documents that carry this tag.
pub async fn tagged_with(
    connection: &mut SqliteConnection,
    tag_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT document_id FROM document_tags WHERE tag_id = ?1 ORDER BY document_id",
    )
    .bind(tag_id)
    .fetch_all(&mut *connection)
    .await
}

pub async fn replace_document_tags(
    connection: &mut SqliteConnection,
    document_id: &str,
    rows: &[DocumentTagRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM document_tags WHERE document_id = ?1")
        .bind(document_id)
        .execute(&mut *connection)
        .await?;
    for row in rows {
        sqlx::query(
            "INSERT INTO document_tags (document_id, tag_id, created_at) VALUES (?1, ?2, ?3)",
        )
        .bind(document_id)
        .bind(&row.tag_id)
        .bind(&row.created_at)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

// --- Documents -------------------------------------------------------------------

pub async fn document_title(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT title FROM documents WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
}

pub async fn document_state(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<Option<DocumentState>, sqlx::Error> {
    let row = sqlx::query("SELECT deleted_at, archived_at FROM documents WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?;
    row.map(|row| {
        Ok(DocumentState {
            deleted_at: row.try_get("deleted_at")?,
            archived_at: row.try_get("archived_at")?,
        })
    })
    .transpose()
}

pub async fn set_document_state(
    connection: &mut SqliteConnection,
    id: &str,
    state: &DocumentState,
) -> Result<u64, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE documents SET deleted_at = ?2, archived_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(&state.deleted_at)
            .bind(&state.archived_at)
            .execute(&mut *connection)
            .await?
            .rows_affected(),
    )
}
