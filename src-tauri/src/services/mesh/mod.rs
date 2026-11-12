//! The Mesh of one open library (section 15, ADR 0022): the views, the
//! confirmed connections and the positions of each view. Suggestions of
//! the grouping analysis are in [`suggestions`].

pub mod graph;
pub mod suggestions;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::clock::Generators;
use crate::database::Database;
use crate::database::mesh::{self as rows, ConnectionRow, PositionRow};
pub use graph::{
    GraphQuery, MAX_DRAWN_NODES, MeshCrumb, MeshData, MeshEdge, MeshEdgeKind, MeshGraph, MeshNode,
    MeshNodeKind, MeshView, UNASSIGNED_ID,
};
use suggestions::{
    AnalysisElement, AnalysisProgress, AnalysisRegistry, AnalysisReport, AnalysisStep,
    SuggestionReason,
};

/// Positions one call may save.
pub const MAX_POSITIONS: usize = 5000;
pub const MAX_NOTE_CHARS: usize = 2000;

#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("no such node")]
    NodeNotFound,
    #[error("no such connection")]
    ConnectionNotFound,
    #[error("already connected")]
    ConnectionExists,
    #[error("a node cannot connect to itself")]
    SameNode,
    #[error("no such suggestion")]
    SuggestionNotFound,
    #[error("the analysis is already running")]
    AnalysisRunning,
    #[error("the analysis was cancelled")]
    Cancelled,
}

/// A confirmed connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshConnection {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub kind: MeshEdgeKind,
    pub note: Option<String>,
    pub created_at: String,
}

impl From<ConnectionRow> for MeshConnection {
    fn from(row: ConnectionRow) -> Self {
        Self {
            kind: if row.kind == "similarity" {
                MeshEdgeKind::Similarity
            } else {
                MeshEdgeKind::Manual
            },
            id: row.id,
            node_a: row.node_a,
            node_b: row.node_b,
            note: row.note,
            created_at: row.created_at,
        }
    }
}

/// A position to save.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshPosition {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub pinned: bool,
}

/// A suggestion with the titles of both ends.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshSuggestion {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub title_a: String,
    pub title_b: String,
    pub kind_a: MeshNodeKind,
    pub kind_b: MeshNodeKind,
    pub score: f64,
    pub reason: SuggestionReason,
}

/// "Annehmen" or "Ablehnen"; "Später" leaves a suggestion open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SuggestionDecision {
    Accept,
    Reject,
}

/// The Mesh of one open library.
#[derive(Debug, Clone)]
pub struct Mesh {
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    analyses: Arc<AnalysisRegistry>,
}

fn sorted(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

impl Mesh {
    pub(crate) fn new(
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
        analyses: Arc<AnalysisRegistry>,
    ) -> Self {
        Self {
            database,
            generators,
            write_lock,
            analyses,
        }
    }

    /// Documents and media outside the trash and the archive, with what
    /// the analysis compares.
    pub async fn analysis_input(&self) -> Result<Vec<AnalysisElement>, MeshError> {
        let pool = self.database.reader();
        let mut elements: Vec<AnalysisElement> = Vec::new();
        let mut position: HashMap<String, usize> = HashMap::new();
        for row in rows::elements(pool)
            .await?
            .into_iter()
            .filter(|e| !e.archived)
        {
            position.insert(row.id.clone(), elements.len());
            elements.push(AnalysisElement {
                id: row.id,
                title: row.title,
                import_folder: row.import_folder.filter(|f| !f.is_empty()),
                ..AnalysisElement::default()
            });
        }
        for (element, tag) in rows::element_tags(pool).await? {
            if let Some(&i) = position.get(&element) {
                elements[i].tags.push(tag);
            }
        }
        for member in rows::members(pool).await? {
            if let Some(&i) = position.get(&member.element_id) {
                elements[i].subnets.push(member.subnet_id);
            }
        }
        for (from, to) in rows::references(pool).await? {
            if let Some(&i) = position.get(&from) {
                elements[i].links.push(to);
            }
        }
        for (document, text) in rows::document_texts(pool).await? {
            if let Some(&i) = position.get(&document) {
                elements[i].text = text;
            }
        }
        Ok(elements)
    }

    /// Runs the grouping analysis in the background (section 15) and stores
    /// what it finds in `mesh_suggestions`; never touches confirmed
    /// connections. Cancelled, it stores nothing.
    pub async fn analyze(
        &self,
        threshold: f64,
        progress: Arc<dyn Fn(AnalysisProgress) + Send + Sync>,
    ) -> Result<AnalysisReport, MeshError> {
        let cancel = self.analyses.start().ok_or(MeshError::AnalysisRunning)?;
        let result = self.run_analysis(threshold, cancel, progress).await;
        self.analyses.finish();
        result
    }

    async fn run_analysis(
        &self,
        threshold: f64,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        progress: Arc<dyn Fn(AnalysisProgress) + Send + Sync>,
    ) -> Result<AnalysisReport, MeshError> {
        progress(AnalysisProgress {
            step: AnalysisStep::Reading,
            done: 0,
            total: 1,
        });
        let elements = self.analysis_input().await?;
        let known: HashSet<(String, String)> = rows::known_pairs(self.database.reader())
            .await?
            .into_iter()
            .collect();
        let count = u32::try_from(elements.len()).unwrap_or(u32::MAX);
        let flag = cancel.clone();
        let report = progress.clone();
        let found = tokio::task::spawn_blocking(move || {
            suggestions::analyze(&elements, &known, threshold, &flag, &*report)
        })
        .await
        .map_err(|_| MeshError::Cancelled)?;
        let Some((found, compared)) = found else {
            return Err(MeshError::Cancelled);
        };
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(MeshError::Cancelled);
        }
        progress(AnalysisProgress {
            step: AnalysisStep::Saving,
            done: 0,
            total: u32::try_from(found.len()).unwrap_or(u32::MAX),
        });
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        let now = self.generators.now().to_iso();
        let mut created = 0;
        for candidate in &found {
            let row = rows::SuggestionRow {
                id: self.generators.new_id().hyphenated().to_string(),
                node_a: candidate.node_a.clone(),
                node_b: candidate.node_b.clone(),
                score: candidate.score,
                reason: serde_json::to_string(&candidate.reason)
                    .unwrap_or_else(|_| "{}".to_owned()),
                status: "open".to_owned(),
                created_at: now.clone(),
                decided_at: None,
            };
            if rows::insert_suggestion(&mut tx, &row).await? {
                created += 1;
            }
        }
        tx.commit().await?;
        Ok(AnalysisReport {
            elements: count,
            pairs_compared: compared,
            created,
        })
    }

    fn settings(&self) -> crate::services::settings::library::LibrarySettings {
        crate::services::settings::library::LibrarySettings::new(
            self.database.clone(),
            self.generators.clone(),
        )
    }

    /// The threshold of the analysis (library setting).
    pub async fn threshold(&self) -> f64 {
        self.settings()
            .get::<crate::services::settings::library::MeshSuggestionThreshold>()
            .await
            .unwrap_or(suggestions::DEFAULT_THRESHOLD)
    }

    /// Changes the threshold; `false` if the value is out of range.
    pub async fn set_threshold(&self, value: f64) -> bool {
        self.settings()
            .set::<crate::services::settings::library::MeshSuggestionThreshold>(&value)
            .await
            .is_ok()
    }

    /// Cancels a running analysis; `false` if none runs.
    pub fn cancel_analysis(&self) -> bool {
        self.analyses.cancel()
    }

    /// Open suggestions, the best first.
    pub async fn suggestions(&self, limit: u32) -> Result<Vec<MeshSuggestion>, MeshError> {
        let pool = self.database.reader();
        let open = rows::open_suggestions(pool, i64::from(limit)).await?;
        if open.is_empty() {
            return Ok(Vec::new());
        }
        let titles: HashMap<String, (String, String)> = rows::node_titles(pool)
            .await?
            .into_iter()
            .map(|(id, kind, title)| (id, (kind, title)))
            .collect();
        let kind_of = |kind: &str| match kind {
            "media" => MeshNodeKind::Media,
            "subnet" => MeshNodeKind::Subnet,
            _ => MeshNodeKind::Document,
        };
        Ok(open
            .into_iter()
            .map(|row| {
                let (kind_a, title_a) = titles.get(&row.node_a).cloned().unwrap_or_default();
                let (kind_b, title_b) = titles.get(&row.node_b).cloned().unwrap_or_default();
                MeshSuggestion {
                    reason: serde_json::from_str(&row.reason).unwrap_or_default(),
                    id: row.id,
                    node_a: row.node_a,
                    node_b: row.node_b,
                    title_a,
                    title_b,
                    kind_a: kind_of(&kind_a),
                    kind_b: kind_of(&kind_b),
                    score: row.score,
                }
            })
            .collect())
    }

    /// Every suggestion with its status, for checks and tests.
    pub async fn all_suggestions(&self) -> Result<Vec<(String, String, String)>, MeshError> {
        Ok(rows::all_suggestions(self.database.reader())
            .await?
            .into_iter()
            .map(|row| (row.node_a, row.node_b, row.status))
            .collect())
    }

    /// Accepts (a connection of kind `similarity`) or rejects a suggestion;
    /// a rejected pair keeps its row and is never suggested again.
    pub async fn decide(
        &self,
        id: &str,
        decision: SuggestionDecision,
    ) -> Result<Option<MeshConnection>, MeshError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        let row = rows::suggestion(&mut tx, id)
            .await?
            .filter(|row| row.status == "open")
            .ok_or(MeshError::SuggestionNotFound)?;
        let now = self.generators.now().to_iso();
        let status = match decision {
            SuggestionDecision::Accept => "accepted",
            SuggestionDecision::Reject => "rejected",
        };
        rows::decide_suggestion(&mut tx, id, status, &now).await?;
        let mut made = None;
        if decision == SuggestionDecision::Accept
            && rows::connection_between(&mut tx, &row.node_a, &row.node_b, "similarity")
                .await?
                .is_none()
        {
            let connection = ConnectionRow {
                id: self.generators.new_id().hyphenated().to_string(),
                node_a: row.node_a,
                node_b: row.node_b,
                kind: "similarity".to_owned(),
                note: None,
                created_at: now,
            };
            rows::insert_connection(&mut tx, &connection).await?;
            made = Some(connection.into());
        }
        tx.commit().await?;
        Ok(made)
    }

    /// Everything a view is built from.
    pub async fn data(&self) -> Result<MeshData, MeshError> {
        let pool = self.database.reader();
        Ok(MeshData {
            elements: rows::elements(pool).await?,
            subnets: rows::subnets(pool).await?,
            members: rows::members(pool).await?,
            references: rows::references(pool).await?,
            connections: rows::connections(pool).await?,
        })
    }

    /// A view, with the positions stored for it.
    pub async fn graph(&self, query: &GraphQuery) -> Result<MeshGraph, MeshError> {
        let data = self.data().await?;
        let positions =
            match rows::layout_id(self.database.reader(), query.view.as_str(), &query.scope_id)
                .await?
            {
                Some(layout) => rows::positions(self.database.reader(), &layout).await?,
                None => Vec::new(),
            };
        Ok(graph::build(&data, query, &positions))
    }

    /// Confirmed connections, for checks and tests.
    pub async fn connections(&self) -> Result<Vec<MeshConnection>, MeshError> {
        Ok(rows::connections(self.database.reader())
            .await?
            .into_iter()
            .map(MeshConnection::from)
            .collect())
    }

    /// Draws a connection between two nodes ("Griff am Knoten ziehen").
    pub async fn connect(
        &self,
        a: &str,
        b: &str,
        note: Option<&str>,
    ) -> Result<MeshConnection, MeshError> {
        self.insert_connection(a, b, "manual", note, None).await
    }

    /// Writes a connection; `id` given restores a removed one (undo).
    pub(crate) async fn insert_connection(
        &self,
        a: &str,
        b: &str,
        kind: &str,
        note: Option<&str>,
        id: Option<&str>,
    ) -> Result<MeshConnection, MeshError> {
        if a == b {
            return Err(MeshError::SameNode);
        }
        let (node_a, node_b) = sorted(a, b);
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        for node in [&node_a, &node_b] {
            if !rows::node_exists(&mut tx, node).await? {
                return Err(MeshError::NodeNotFound);
            }
        }
        if rows::connection_between(&mut tx, &node_a, &node_b, kind)
            .await?
            .is_some()
        {
            return Err(MeshError::ConnectionExists);
        }
        let row = ConnectionRow {
            id: id.map_or_else(
                || self.generators.new_id().hyphenated().to_string(),
                str::to_owned,
            ),
            node_a,
            node_b,
            kind: kind.to_owned(),
            note: note.map(str::to_owned),
            created_at: self.generators.now().to_iso(),
        };
        rows::insert_connection(&mut tx, &row).await?;
        tx.commit().await?;
        Ok(row.into())
    }

    /// Restores a removed connection with its id (undo of `disconnect`).
    pub async fn restore(&self, connection: &MeshConnection) -> Result<MeshConnection, MeshError> {
        let kind = if connection.kind == MeshEdgeKind::Similarity {
            "similarity"
        } else {
            "manual"
        };
        self.insert_connection(
            &connection.node_a,
            &connection.node_b,
            kind,
            connection.note.as_deref(),
            Some(&connection.id),
        )
        .await
    }

    /// Removes a confirmed connection (`Entf`); returns it for undo.
    pub async fn disconnect(&self, id: &str) -> Result<MeshConnection, MeshError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        let row = rows::connection(&mut tx, id)
            .await?
            .ok_or(MeshError::ConnectionNotFound)?;
        rows::delete_connection(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row.into())
    }

    /// Saves positions of a view; `remove` drops stored ones (undo of an
    /// automatic layout). Positions of nodes that no longer exist are
    /// skipped.
    pub async fn save_positions(
        &self,
        view: MeshView,
        scope_id: &str,
        positions: &[MeshPosition],
        remove: &[String],
        algorithm: Option<&str>,
    ) -> Result<(), MeshError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        let layout = rows::ensure_layout(
            &mut tx,
            &self.generators.new_id().hyphenated().to_string(),
            view.as_str(),
            scope_id,
            algorithm,
            &self.generators.now().to_iso(),
        )
        .await?;
        let wanted: BTreeSet<&str> = positions.iter().map(|p| p.node_id.as_str()).collect();
        for node in remove {
            if !wanted.contains(node.as_str()) {
                rows::delete_position(&mut tx, &layout, node).await?;
            }
        }
        for position in positions {
            if !rows::node_exists(&mut tx, &position.node_id).await? {
                continue;
            }
            rows::upsert_position(
                &mut tx,
                &layout,
                &PositionRow {
                    node_id: position.node_id.clone(),
                    x: position.x,
                    y: position.y,
                    pinned: position.pinned,
                },
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
