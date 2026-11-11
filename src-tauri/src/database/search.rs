//! SQL for the search index (migration 0009, section 19, ADR 0020).
//!
//! The triggers of migration 0009 keep titles, tags, subnets and metadata
//! current. This module writes the body of a document (from its file, in the
//! transaction that records the content), finds stale bodies, rebuilds the
//! index and runs the two queries: words first, then parts of words.

use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

/// Marks a hit in a snippet: U+E000 before, U+E001 after. Both are private
/// use characters; indexed text never contains them (see
/// `services::search::text`).
pub const HIT_START: char = '\u{e000}';
pub const HIT_END: char = '\u{e001}';

/// Writes the body of a document and the checksum of the content it comes
/// from. Called in the transaction that records the content.
pub async fn set_document_body(
    connection: &mut SqliteConnection,
    document_id: &str,
    body: &str,
    content_sha256: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE search_words SET body = ?2 \
         WHERE rowid = (SELECT id FROM search_entries WHERE document_id = ?1)",
    )
    .bind(document_id)
    .bind(body)
    .execute(&mut *connection)
    .await?;
    sqlx::query(
        "UPDATE search_parts SET body = ?2 \
         WHERE rowid = (SELECT id FROM search_entries WHERE document_id = ?1)",
    )
    .bind(document_id)
    .bind(body)
    .execute(&mut *connection)
    .await?;
    sqlx::query("UPDATE search_entries SET indexed_sha256 = ?2 WHERE document_id = ?1")
        .bind(document_id)
        .bind(content_sha256)
        .execute(&mut *connection)
        .await?;
    Ok(())
}

/// A document whose body is missing from the index or older than its
/// content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleDocument {
    pub id: String,
    pub format: String,
    pub content_sha256: String,
}

/// Documents whose indexed body does not match their content.
pub async fn stale_documents(pool: &SqlitePool) -> Result<Vec<StaleDocument>, sqlx::Error> {
    sqlx::query(
        "SELECT d.id, d.format, d.content_sha256 FROM documents AS d \
         JOIN search_entries AS e ON e.document_id = d.id \
         WHERE e.indexed_sha256 IS NOT d.content_sha256 \
         ORDER BY d.id",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| {
        Ok(StaleDocument {
            id: row.try_get("id")?,
            format: row.try_get("format")?,
            content_sha256: row.try_get("content_sha256")?,
        })
    })
    .collect()
}

/// Builds the index anew from the tables ("Suchindex neu aufbauen"). The
/// bodies of documents are empty afterwards and marked stale; the caller
/// fills them in from the files.
pub async fn rebuild(connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    // Without the entry trigger: the FTS tables are emptied as a whole.
    sqlx::query("DELETE FROM search_words")
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM search_parts")
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM search_entries")
        .execute(&mut *connection)
        .await?;
    for statement in [
        "INSERT INTO search_entries (document_id) SELECT id FROM documents ORDER BY created_at, id",
        "INSERT INTO search_entries (media_id) SELECT id FROM media ORDER BY created_at, id",
        "INSERT INTO search_entries (subnet_id) SELECT id FROM subnets ORDER BY created_at, id",
        "INSERT INTO search_words (rowid, title, body, tags, subnets, meta) \
         SELECT e.id, f.title, '', f.tags, f.subnets, f.meta \
         FROM search_entries AS e JOIN search_document_fields AS f ON f.id = e.document_id",
        "INSERT INTO search_words (rowid, title, body, tags, subnets, meta) \
         SELECT e.id, f.title, f.body, f.tags, f.subnets, f.meta \
         FROM search_entries AS e JOIN search_media_fields AS f ON f.id = e.media_id",
        "INSERT INTO search_words (rowid, title, body, tags, subnets, meta) \
         SELECT e.id, f.title, f.body, '', f.subnets, '' \
         FROM search_entries AS e JOIN search_subnet_fields AS f ON f.id = e.subnet_id",
        "INSERT INTO search_parts (rowid, title, body) \
         SELECT e.id, f.title, '' FROM search_entries AS e \
         JOIN search_document_fields AS f ON f.id = e.document_id",
        "INSERT INTO search_parts (rowid, title, body) \
         SELECT e.id, f.title, f.body FROM search_entries AS e \
         JOIN search_media_fields AS f ON f.id = e.media_id",
        "INSERT INTO search_parts (rowid, title, body) \
         SELECT e.id, f.title, f.body FROM search_entries AS e \
         JOIN search_subnet_fields AS f ON f.id = e.subnet_id",
    ] {
        sqlx::query(statement).execute(&mut *connection).await?;
    }
    Ok(())
}

/// Filters of a search, as bind values (`None` = no filter).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    /// JSON array of kinds: `document`, `image`, `pdf`, `attachment`, `subnet`.
    pub kinds: Option<String>,
    /// JSON array of tag ids; every one must be on the element.
    pub tags: Option<String>,
    /// The number of tags in `tags`.
    pub tag_count: i64,
    /// A subnet; elements in it or below it, and the subnets below it.
    pub subnet: Option<String>,
    /// `updated_at` from (inclusive) and to (exclusive), ISO 8601.
    pub updated_from: Option<String>,
    pub updated_before: Option<String>,
    pub favorites_only: bool,
    pub include_archived: bool,
    pub include_trash: bool,
}

/// One hit, as the query returns it.
#[derive(Debug, Clone, PartialEq)]
pub struct HitRow {
    pub entry: i64,
    pub document_id: Option<String>,
    pub media_id: Option<String>,
    pub subnet_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub format: Option<String>,
    pub updated_at: String,
    pub archived: bool,
    pub deleted: bool,
    pub favorite: bool,
    pub snippet: String,
    pub rank: f64,
}

fn hit_from_row(row: &SqliteRow) -> Result<HitRow, sqlx::Error> {
    Ok(HitRow {
        entry: row.try_get("entry")?,
        document_id: row.try_get("document_id")?,
        media_id: row.try_get("media_id")?,
        subnet_id: row.try_get("subnet_id")?,
        kind: row.try_get("kind")?,
        title: row.try_get("title")?,
        format: row.try_get("format")?,
        updated_at: row.try_get("updated_at")?,
        archived: row.try_get::<i64, _>("archived")? != 0,
        deleted: row.try_get::<i64, _>("deleted")? != 0,
        favorite: row.try_get::<i64, _>("favorite")? != 0,
        snippet: row.try_get("snippet")?,
        rank: row.try_get("rank")?,
    })
}

// The element behind an entry and the filters, shared by both queries.
// Parameters: ?2 kinds, ?3 tags, ?4 tag count, ?5 subnet, ?6 from,
// ?7 before, ?8 favourites only, ?9 archive, ?10 trash, ?11 limit.
macro_rules! element_and_filters {
    () => {
        " JOIN search_entries AS e ON e.id = hit.rowid \
          LEFT JOIN documents AS d ON d.id = e.document_id \
          LEFT JOIN media AS m ON m.id = e.media_id \
          LEFT JOIN subnets AS s ON s.id = e.subnet_id \
          ) AS r WHERE (?2 IS NULL OR r.kind IN (SELECT value FROM json_each(?2))) \
          AND (?3 IS NULL OR ?4 = (SELECT count(DISTINCT g.value) FROM json_each(?3) AS g \
               WHERE EXISTS (SELECT 1 FROM document_tags AS dt WHERE dt.document_id = r.document_id AND dt.tag_id = g.value) \
                  OR EXISTS (SELECT 1 FROM media_tags AS mt WHERE mt.media_id = r.media_id AND mt.tag_id = g.value))) \
          AND (?5 IS NULL OR EXISTS ( \
               WITH RECURSIVE scope (id) AS ( \
                 SELECT ?5 UNION SELECT c.id FROM subnets AS c JOIN scope ON c.parent_id = scope.id) \
               SELECT 1 FROM scope WHERE scope.id = r.subnet_id \
                  OR EXISTS (SELECT 1 FROM subnet_memberships AS sm WHERE sm.subnet_id = scope.id \
                             AND (sm.document_id = r.document_id OR sm.media_id = r.media_id)))) \
          AND (?6 IS NULL OR r.updated_at >= ?6) \
          AND (?7 IS NULL OR r.updated_at < ?7) \
          AND (?8 = 0 OR r.favorite = 1) \
          AND (?9 = 1 OR r.archived = 0) \
          AND (?10 = 1 OR r.deleted = 0) \
          ORDER BY r.rank, r.entry LIMIT ?11"
    };
}

macro_rules! element_columns {
    () => {
        "e.id AS entry, e.document_id AS document_id, e.media_id AS media_id, e.subnet_id AS subnet_id, \
         CASE WHEN e.document_id IS NOT NULL THEN 'document' \
              WHEN e.media_id IS NOT NULL THEN m.kind ELSE 'subnet' END AS kind, \
         coalesce(d.title, m.title, s.name) AS title, d.format AS format, \
         coalesce(d.updated_at, m.updated_at, s.updated_at) AS updated_at, \
         coalesce(d.archived_at, m.archived_at) IS NOT NULL AS archived, \
         coalesce(d.deleted_at, m.deleted_at) IS NOT NULL AS deleted, \
         coalesce(d.favorited_at, m.favorited_at) IS NOT NULL AS favorite, "
    };
}

const WORDS_SQL: &str = concat!(
    "SELECT * FROM (SELECT ",
    element_columns!(),
    "hit.snippet AS snippet, hit.rank AS rank FROM ( \
       SELECT rowid, \
              snippet(search_words, -1, char(57344), char(57345), '…', 16) AS snippet, \
              bm25(search_words, 10.0, 1.0, 5.0, 3.0, 2.0) AS rank \
       FROM search_words WHERE search_words MATCH ?1 \
       AND (?12 IS NULL OR rowid IN (SELECT rowid FROM search_parts WHERE search_parts MATCH ?12))) AS hit",
    element_and_filters!()
);

// ?12 parts of words, ?13 short words (below three characters) that must
// match as words too, ?1 and ?14 the words query whose hits are already
// listed (with its exact words).
const PARTS_SQL: &str = concat!(
    "SELECT * FROM (SELECT ",
    element_columns!(),
    "hit.snippet AS snippet, hit.rank AS rank FROM ( \
       SELECT rowid, \
              snippet(search_parts, -1, char(57344), char(57345), '…', 48) AS snippet, \
              bm25(search_parts, 10.0, 1.0) AS rank \
       FROM search_parts WHERE search_parts MATCH ?12 \
       AND (?13 IS NULL OR rowid IN (SELECT rowid FROM search_words WHERE search_words MATCH ?13)) \
       AND (?1 IS NULL OR rowid NOT IN (SELECT rowid FROM search_words WHERE search_words MATCH ?1 \
            AND (?14 IS NULL OR rowid IN (SELECT rowid FROM search_parts WHERE search_parts MATCH ?14))))) AS hit",
    element_and_filters!()
);

fn bind_filters<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    filters: &'q Filters,
    limit: i64,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    query
        .bind(&filters.kinds)
        .bind(&filters.tags)
        .bind(filters.tag_count)
        .bind(&filters.subnet)
        .bind(&filters.updated_from)
        .bind(&filters.updated_before)
        .bind(i64::from(filters.favorites_only))
        .bind(i64::from(filters.include_archived))
        .bind(i64::from(filters.include_trash))
        .bind(limit)
}

/// Hits of the words index (`search_words`), best first; `exact` (for the
/// parts index) must match as well.
pub async fn search_words(
    pool: &SqlitePool,
    words: &str,
    exact: Option<&str>,
    filters: &Filters,
    limit: i64,
) -> Result<Vec<HitRow>, sqlx::Error> {
    bind_filters(sqlx::query(WORDS_SQL).bind(words), filters, limit)
        .bind(exact)
        .fetch_all(pool)
        .await?
        .iter()
        .map(hit_from_row)
        .collect()
}

/// Hits of the parts index (`search_parts`) that the words query did not
/// find, best first.
pub async fn search_parts(
    pool: &SqlitePool,
    words: Option<&str>,
    exact: Option<&str>,
    parts: &str,
    short_words: Option<&str>,
    filters: &Filters,
    limit: i64,
) -> Result<Vec<HitRow>, sqlx::Error> {
    bind_filters(sqlx::query(PARTS_SQL).bind(words), filters, limit)
        .bind(parts)
        .bind(short_words)
        .bind(exact)
        .fetch_all(pool)
        .await?
        .iter()
        .map(hit_from_row)
        .collect()
}

/// Number of entries and of FTS rows in each table (for checks and tests).
pub async fn counts(pool: &SqlitePool) -> Result<(i64, i64, i64), sqlx::Error> {
    let entries = sqlx::query_scalar("SELECT count(*) FROM search_entries")
        .fetch_one(pool)
        .await?;
    let words = sqlx::query_scalar("SELECT count(*) FROM search_words")
        .fetch_one(pool)
        .await?;
    let parts = sqlx::query_scalar("SELECT count(*) FROM search_parts")
        .fetch_one(pool)
        .await?;
    Ok((entries, words, parts))
}
