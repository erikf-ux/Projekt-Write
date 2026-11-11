//! Global search (Phase 9, section 19, ADR 0020): one query with filters,
//! and "Suchindex neu aufbauen".

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::Timestamp;
use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::search::{
    DEFAULT_HITS, MAX_HITS, MAX_QUERY_CHARS, SearchError, SearchHit, SearchKind, SearchOptions,
};
use crate::state::AppState;

/// Tags of one search at most.
const MAX_TAG_FILTERS: usize = 50;

pub(crate) fn search_error(error: SearchError) -> CommandError {
    match error {
        SearchError::Database(error) => {
            log::error!("search database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        SearchError::Path(error) => {
            log::error!("search path error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        SearchError::Io(error) => {
            log::error!("search file error: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
    }
}

fn parse_uuid(field: &str, id: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string() == id)
        .ok_or_else(|| CommandError::invalid_input(field, "not an id"))
}

fn parse_time(field: &str, value: &str) -> Result<String, CommandError> {
    Timestamp::parse_iso(value)
        .map(|time| time.to_iso())
        .ok_or_else(|| CommandError::invalid_input(field, "not an ISO 8601 time"))
}

/// Request of `search_query`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SearchRequest {
    pub query: String,
    /// Empty or missing: every kind.
    #[serde(default)]
    #[ts(optional)]
    pub kinds: Option<Vec<SearchKind>>,
    /// Every tag must be on the element.
    #[serde(default)]
    #[ts(optional)]
    pub tag_ids: Option<Vec<String>>,
    /// Elements in this subnet or below it.
    #[serde(default)]
    #[ts(optional)]
    pub subnet_id: Option<String>,
    /// Last change from (inclusive), ISO 8601 UTC.
    #[serde(default)]
    #[ts(optional)]
    pub updated_from: Option<String>,
    /// Last change before (exclusive), ISO 8601 UTC.
    #[serde(default)]
    #[ts(optional)]
    pub updated_before: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub favorites_only: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub include_archived: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub include_trash: Option<bool>,
    /// 1 to 200; 50 if missing.
    #[serde(default)]
    #[ts(optional)]
    pub limit: Option<u32>,
}

impl SearchRequest {
    fn options(&self) -> Result<SearchOptions, CommandError> {
        let tags = self
            .tag_ids
            .iter()
            .flatten()
            .map(|id| parse_uuid("tagIds", id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SearchOptions {
            kinds: self.kinds.clone().unwrap_or_default(),
            tags,
            subnet: self
                .subnet_id
                .as_deref()
                .map(|id| parse_uuid("subnetId", id))
                .transpose()?,
            updated_from: self
                .updated_from
                .as_deref()
                .map(|value| parse_time("updatedFrom", value))
                .transpose()?,
            updated_before: self
                .updated_before
                .as_deref()
                .map(|value| parse_time("updatedBefore", value))
                .transpose()?,
            favorites_only: self.favorites_only.unwrap_or(false),
            include_archived: self.include_archived.unwrap_or(false),
            include_trash: self.include_trash.unwrap_or(false),
            limit: self.limit.unwrap_or(DEFAULT_HITS),
        })
    }
}

impl Validate for SearchRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.query.chars().count() > MAX_QUERY_CHARS {
            return Err(CommandError::invalid_input("query", "too long"));
        }
        if self
            .tag_ids
            .as_ref()
            .is_some_and(|tags| tags.len() > MAX_TAG_FILTERS)
        {
            return Err(CommandError::invalid_input("tagIds", "too many"));
        }
        if self
            .limit
            .is_some_and(|limit| !(1..=MAX_HITS).contains(&limit))
        {
            return Err(CommandError::invalid_input("limit", "1 to 200"));
        }
        self.options().map(|_| ())
    }
}

/// Hits of a search: words first, then parts of words.
#[tauri::command]
pub async fn search_query(
    state: State<'_, AppState>,
    request: Input<SearchRequest>,
) -> Result<Vec<SearchHit>, CommandError> {
    let request = request.into_inner();
    let options = request.options()?;
    state
        .session
        .with_search(|search| async move {
            search
                .search(&request.query, &options)
                .await
                .map_err(search_error)
        })
        .await
}

/// Result of `search_rebuild`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebuildReport {
    /// Documents whose text was indexed.
    pub indexed: u32,
    /// Documents whose file could not be read.
    pub unreadable: u32,
}

/// "Suchindex neu aufbauen" (section 19).
#[tauri::command]
pub async fn search_rebuild(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<RebuildReport, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_search(|search| async move {
            let report = search.rebuild().await.map_err(search_error)?;
            Ok(RebuildReport {
                indexed: u32::try_from(report.indexed).unwrap_or(u32::MAX),
                unreadable: u32::try_from(report.unreadable).unwrap_or(u32::MAX),
            })
        })
        .await
}
