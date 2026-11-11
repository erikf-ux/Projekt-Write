//! Global search of one open library (section 19, ADR 0020).
//!
//! The index lives in the library database (migration 0009). Triggers keep
//! what the database knows current; the bodies of documents come from their
//! files, written by the document repository in the transaction of the new
//! content. This service
//!
//! * fills in bodies that are missing or stale (after the migration, after a
//!   rebuild),
//! * rebuilds the index ("Suchindex neu aufbauen"),
//! * searches: words first, then parts of words, each ranked with `bm25()`.

pub mod query;
pub mod text;

use std::io;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::database::Database;
use crate::database::search::{self as rows, Filters, HIT_END, HIT_START, HitRow};
use crate::filesystem::atomic::read_limited;
use crate::security::paths::{PathError, SafeRoot};
use crate::services::documents::{DocumentFormat, MAX_DOCUMENT_BYTES};

/// Hits of one search at most.
pub const MAX_HITS: u32 = 200;
/// Hits of one search unless the request asks for fewer.
pub const DEFAULT_HITS: u32 = 50;
/// Characters of a search at most.
pub const MAX_QUERY_CHARS: usize = 1_000;
/// Bodies written per transaction when filling in.
const FILL_BATCH: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("path error: {0}")]
    Path(#[from] PathError),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
}

/// What a hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchKind {
    Document,
    Image,
    Pdf,
    Attachment,
    Subnet,
}

impl SearchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SearchKind::Document => "document",
            SearchKind::Image => "image",
            SearchKind::Pdf => "pdf",
            SearchKind::Attachment => "attachment",
            SearchKind::Subnet => "subnet",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [
            SearchKind::Document,
            SearchKind::Image,
            SearchKind::Pdf,
            SearchKind::Attachment,
            SearchKind::Subnet,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == value)
    }
}

/// Which index found a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchMatch {
    /// A word or the beginning of one (`search_words`).
    Words,
    /// A part of a word (`search_parts`).
    Parts,
}

/// A piece of a snippet; `hit` pieces show what matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SnippetPart {
    pub text: String,
    pub hit: bool,
}

/// One hit (no path, no content beyond the snippet).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub title: String,
    /// `txt`, `md` or `pwrite` for documents.
    pub format: Option<String>,
    pub updated_at: String,
    pub archived: bool,
    pub deleted: bool,
    pub favorite: bool,
    pub snippet: Vec<SnippetPart>,
    #[serde(rename = "match")]
    #[ts(rename = "match")]
    pub matched: SearchMatch,
}

/// Filters of a search (section 19: Typ, Tags, Subnetz, Datum, Favoriten,
/// Archiv einbeziehen; the trash as well).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Empty: every kind.
    pub kinds: Vec<SearchKind>,
    /// Every tag must be on the element.
    pub tags: Vec<Uuid>,
    /// Elements in this subnet or below it, and the subnets below it.
    pub subnet: Option<Uuid>,
    /// Last change from (inclusive) and before (exclusive), ISO 8601 UTC.
    pub updated_from: Option<String>,
    pub updated_before: Option<String>,
    pub favorites_only: bool,
    pub include_archived: bool,
    pub include_trash: bool,
    pub limit: u32,
}

/// Outcome of filling in bodies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FillReport {
    /// Bodies written.
    pub indexed: usize,
    /// Documents whose file could not be read; they stay stale and are
    /// tried again at the next opening.
    pub unreadable: usize,
}

/// The search index of one library.
#[derive(Clone)]
pub struct SearchIndex {
    root: SafeRoot,
    database: Database,
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SearchIndex {
    pub(crate) fn new(
        root: SafeRoot,
        database: Database,
        write_lock: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            root,
            database,
            write_lock,
        }
    }

    /// Writes the bodies of documents whose index does not match their
    /// content. Holds the write lock, so no save comes in between.
    pub async fn fill_stale(&self) -> Result<FillReport, SearchError> {
        let _guard = self.write_lock.lock().await;
        self.fill_stale_locked().await
    }

    async fn fill_stale_locked(&self) -> Result<FillReport, SearchError> {
        let stale = rows::stale_documents(self.database.writer()).await?;
        let mut report = FillReport::default();
        for batch in stale.chunks(FILL_BATCH) {
            let mut bodies = Vec::with_capacity(batch.len());
            for document in batch {
                match self.read_body(document) {
                    Ok(body) => bodies.push((document, body)),
                    Err(error) => {
                        log::warn!("search index: document {} not read: {error}", document.id);
                        report.unreadable += 1;
                    }
                }
            }
            let mut transaction = self.database.writer().begin().await?;
            for (document, body) in &bodies {
                rows::set_document_body(
                    &mut transaction,
                    &document.id,
                    body,
                    &document.content_sha256,
                )
                .await?;
            }
            transaction.commit().await?;
            report.indexed += bodies.len();
        }
        if report.indexed > 0 || report.unreadable > 0 {
            log::info!(
                "search index: {} bodies written, {} files not read",
                report.indexed,
                report.unreadable
            );
        }
        Ok(report)
    }

    fn read_body(&self, document: &rows::StaleDocument) -> Result<String, SearchError> {
        let format = DocumentFormat::parse(&document.format)
            .ok_or_else(|| io::Error::other("unknown format"))?;
        let id = Uuid::try_parse(&document.id).map_err(io::Error::other)?;
        let path = self.root.resolve(&format.relative_path(id))?;
        let content = read_limited(&path, MAX_DOCUMENT_BYTES)?;
        Ok(text::searchable_text(format, &content))
    }

    /// "Suchindex neu aufbauen": the index anew from the tables and files.
    pub async fn rebuild(&self) -> Result<FillReport, SearchError> {
        let _guard = self.write_lock.lock().await;
        let mut transaction = self.database.writer().begin().await?;
        rows::rebuild(&mut transaction).await?;
        transaction.commit().await?;
        self.fill_stale_locked().await
    }

    /// Searches: hits of words first, then hits of parts of words.
    pub async fn search(
        &self,
        input: &str,
        options: &SearchOptions,
    ) -> Result<Vec<SearchHit>, SearchError> {
        let input: String = input.chars().take(MAX_QUERY_CHARS).collect();
        let expressions = query::expressions(&input);
        let Some(words) = expressions.words.as_deref() else {
            return Ok(Vec::new());
        };
        let limit = options.limit.clamp(1, MAX_HITS);
        let filters = filters(options);
        let reader = self.database.reader();
        let mut hits: Vec<SearchHit> =
            rows::search_words(reader, words, &filters, i64::from(limit))
                .await?
                .into_iter()
                .filter_map(|row| hit(row, SearchMatch::Words))
                .collect();
        let remaining = limit.saturating_sub(u32::try_from(hits.len()).unwrap_or(limit));
        if remaining > 0
            && let Some(parts) = expressions.parts.as_deref()
        {
            hits.extend(
                rows::search_parts(
                    reader,
                    Some(words),
                    parts,
                    expressions.short_words.as_deref(),
                    &filters,
                    i64::from(remaining),
                )
                .await?
                .into_iter()
                .filter_map(|row| hit(row, SearchMatch::Parts)),
            );
        }
        Ok(hits)
    }
}

fn filters(options: &SearchOptions) -> Filters {
    let json = |values: Vec<&str>| {
        (!values.is_empty()).then(|| serde_json::to_string(&values).unwrap_or_default())
    };
    let tags: Vec<String> = options
        .tags
        .iter()
        .map(|tag| tag.hyphenated().to_string())
        .collect();
    Filters {
        kinds: json(options.kinds.iter().map(|kind| kind.as_str()).collect()),
        tags: json(tags.iter().map(String::as_str).collect()),
        tag_count: i64::try_from(tags.iter().collect::<std::collections::BTreeSet<_>>().len())
            .unwrap_or(i64::MAX),
        subnet: options.subnet.map(|subnet| subnet.hyphenated().to_string()),
        updated_from: options.updated_from.clone(),
        updated_before: options.updated_before.clone(),
        favorites_only: options.favorites_only,
        include_archived: options.include_archived,
        include_trash: options.include_trash,
    }
}

fn hit(row: HitRow, matched: SearchMatch) -> Option<SearchHit> {
    let kind = SearchKind::parse(&row.kind)?;
    let id = row.document_id.or(row.media_id).or(row.subnet_id)?;
    Some(SearchHit {
        kind,
        id,
        title: row.title,
        format: row.format,
        updated_at: row.updated_at,
        archived: row.archived,
        deleted: row.deleted,
        favorite: row.favorite,
        snippet: snippet_parts(&row.snippet),
        matched,
    })
}

/// Splits a snippet at the hit markers. Line breaks become spaces: the
/// snippet is one line in the list.
pub fn snippet_parts(snippet: &str) -> Vec<SnippetPart> {
    let mut parts: Vec<SnippetPart> = Vec::new();
    let mut current = String::new();
    let mut hit = false;
    let flush = |text: &mut String, hit: bool, parts: &mut Vec<SnippetPart>| {
        if !text.is_empty() {
            match parts.last_mut() {
                Some(last) if last.hit == hit => last.text.push_str(text),
                _ => parts.push(SnippetPart {
                    text: std::mem::take(text),
                    hit,
                }),
            }
            text.clear();
        }
    };
    for ch in snippet.chars() {
        match ch {
            HIT_START => {
                flush(&mut current, hit, &mut parts);
                hit = true;
            }
            HIT_END => {
                flush(&mut current, hit, &mut parts);
                hit = false;
            }
            '\n' | '\r' | '\t' => {
                if !current.ends_with(' ') {
                    current.push(' ');
                }
            }
            other => current.push(other),
        }
    }
    flush(&mut current, hit, &mut parts);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_are_split_at_the_markers() {
        let parts = snippet_parts("Herr \u{e000}Müller\u{e001}\nin der \u{e000}Straße\u{e001}…");
        assert_eq!(
            parts,
            [
                SnippetPart {
                    text: "Herr ".into(),
                    hit: false
                },
                SnippetPart {
                    text: "Müller".into(),
                    hit: true
                },
                SnippetPart {
                    text: " in der ".into(),
                    hit: false
                },
                SnippetPart {
                    text: "Straße".into(),
                    hit: true
                },
                SnippetPart {
                    text: "…".into(),
                    hit: false
                },
            ]
        );
        assert!(snippet_parts("").is_empty());
    }

    #[test]
    fn adjacent_hits_are_joined() {
        let parts = snippet_parts("\u{e000}Kranken\u{e001}\u{e000}haus\u{e001}");
        assert_eq!(
            parts,
            [SnippetPart {
                text: "Krankenhaus".into(),
                hit: true
            }]
        );
    }
}
