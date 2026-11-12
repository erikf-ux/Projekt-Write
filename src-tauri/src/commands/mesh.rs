//! The Mesh (Phase 11, section 15, ADR 0022): views, confirmed
//! connections and the positions of each view.

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Channel;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::services::mesh::suggestions::{AnalysisProgress, AnalysisReport};
use crate::services::mesh::{
    GraphQuery, MAX_NOTE_CHARS, MAX_POSITIONS, MeshConnection, MeshEdgeKind, MeshError, MeshGraph,
    MeshPosition, MeshSuggestion, MeshView, SuggestionDecision, UNASSIGNED_ID,
};
use crate::state::AppState;

/// Subnets one view may draw open.
const MAX_EXPANDED: usize = 1000;

pub(crate) fn mesh_error(error: MeshError) -> CommandError {
    match error {
        MeshError::Database(error) => {
            log::error!("mesh database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        MeshError::NodeNotFound => CommandError::new(ErrorCode::MeshNodeNotFound),
        MeshError::ConnectionNotFound => CommandError::new(ErrorCode::MeshConnectionNotFound),
        MeshError::ConnectionExists => CommandError::new(ErrorCode::MeshConnectionExists),
        MeshError::SameNode => CommandError::invalid_input("nodeB", "the same node"),
        MeshError::SuggestionNotFound => CommandError::new(ErrorCode::MeshSuggestionNotFound),
        MeshError::AnalysisRunning => CommandError::new(ErrorCode::MeshAnalysisRunning),
        MeshError::Cancelled => CommandError::new(ErrorCode::MeshAnalysisCancelled),
    }
}

fn is_uuid(id: &str) -> bool {
    Uuid::try_parse(id)
        .ok()
        .is_some_and(|uuid| uuid.hyphenated().to_string() == id)
}

fn check_id(field: &str, id: &str) -> Result<(), CommandError> {
    if is_uuid(id) {
        Ok(())
    } else {
        Err(CommandError::invalid_input(field, "not an id"))
    }
}

/// The scope of a view: none for the Global View, a subnet or a node.
fn check_scope(view: MeshView, scope: &str) -> Result<(), CommandError> {
    match view {
        MeshView::Global if scope.is_empty() => Ok(()),
        MeshView::Global => Err(CommandError::invalid_input(
            "scopeId",
            "empty in the Global View",
        )),
        _ => check_id("scopeId", scope),
    }
}

/// Request of `mesh_graph`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshGraphRequest {
    pub view: MeshView,
    /// The subnet of a Subnet View, the node of a Focus View; empty or
    /// missing in the Global View.
    #[serde(default)]
    #[ts(optional)]
    pub scope_id: Option<String>,
    /// Subnets drawn open (and `unassigned`).
    #[serde(default)]
    #[ts(optional)]
    pub expanded: Option<Vec<String>>,
    /// Focus View: 1 (default) or 2 steps.
    #[serde(default)]
    #[ts(optional)]
    pub depth: Option<u8>,
}

impl MeshGraphRequest {
    fn query(&self) -> GraphQuery {
        GraphQuery {
            view: self.view,
            scope_id: self.scope_id.clone().unwrap_or_default(),
            expanded: self.expanded.iter().flatten().cloned().collect(),
            depth: self.depth.unwrap_or(1),
        }
    }
}

impl Validate for MeshGraphRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_scope(self.view, self.scope_id.as_deref().unwrap_or(""))?;
        let expanded = self.expanded.as_deref().unwrap_or_default();
        if expanded.len() > MAX_EXPANDED {
            return Err(CommandError::invalid_input("expanded", "too many"));
        }
        if expanded
            .iter()
            .any(|id| id != UNASSIGNED_ID && !is_uuid(id))
        {
            return Err(CommandError::invalid_input("expanded", "not an id"));
        }
        if self.depth.is_some_and(|depth| !(1..=2).contains(&depth)) {
            return Err(CommandError::invalid_input("depth", "1 or 2"));
        }
        Ok(())
    }
}

/// A view of the Mesh.
#[tauri::command]
pub async fn mesh_graph(
    state: State<'_, AppState>,
    request: Input<MeshGraphRequest>,
) -> Result<MeshGraph, CommandError> {
    let query = request.into_inner().query();
    state
        .session
        .with_mesh(|mesh, _| async move { mesh.graph(&query).await.map_err(mesh_error) })
        .await
}

/// Request of `mesh_connect`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshConnectRequest {
    pub node_a: String,
    pub node_b: String,
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
}

impl Validate for MeshConnectRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("nodeA", &self.node_a)?;
        check_id("nodeB", &self.node_b)?;
        if self
            .note
            .as_ref()
            .is_some_and(|note| note.chars().count() > MAX_NOTE_CHARS)
        {
            return Err(CommandError::invalid_input("note", "too long"));
        }
        Ok(())
    }
}

/// Draws a manual connection.
#[tauri::command]
pub async fn mesh_connect(
    state: State<'_, AppState>,
    request: Input<MeshConnectRequest>,
) -> Result<MeshConnection, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_mesh(|mesh, _| async move {
            mesh.connect(&request.node_a, &request.node_b, request.note.as_deref())
                .await
                .map_err(mesh_error)
        })
        .await
}

/// Request of `mesh_disconnect`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshConnectionRequest {
    pub id: String,
}

impl Validate for MeshConnectionRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)
    }
}

/// Removes a confirmed connection; returns it, for undo.
#[tauri::command]
pub async fn mesh_disconnect(
    state: State<'_, AppState>,
    request: Input<MeshConnectionRequest>,
) -> Result<MeshConnection, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_mesh(|mesh, _| async move { mesh.disconnect(&request.id).await.map_err(mesh_error) })
        .await
}

/// Request of `mesh_restore_connection` (undo of `mesh_disconnect`).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshRestoreRequest {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub kind: MeshEdgeKind,
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
}

impl Validate for MeshRestoreRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        check_id("nodeA", &self.node_a)?;
        check_id("nodeB", &self.node_b)?;
        if !matches!(self.kind, MeshEdgeKind::Manual | MeshEdgeKind::Similarity) {
            return Err(CommandError::invalid_input("kind", "manual or similarity"));
        }
        if self
            .note
            .as_ref()
            .is_some_and(|note| note.chars().count() > MAX_NOTE_CHARS)
        {
            return Err(CommandError::invalid_input("note", "too long"));
        }
        Ok(())
    }
}

/// Puts a removed connection back, with its id.
#[tauri::command]
pub async fn mesh_restore_connection(
    state: State<'_, AppState>,
    request: Input<MeshRestoreRequest>,
) -> Result<MeshConnection, CommandError> {
    let request = request.into_inner();
    let connection = MeshConnection {
        id: request.id,
        node_a: request.node_a,
        node_b: request.node_b,
        kind: request.kind,
        note: request.note,
        created_at: String::new(),
    };
    state
        .session
        .with_mesh(|mesh, _| async move { mesh.restore(&connection).await.map_err(mesh_error) })
        .await
}

/// Request of `mesh_save_positions`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshPositionsRequest {
    pub view: MeshView,
    #[serde(default)]
    #[ts(optional)]
    pub scope_id: Option<String>,
    pub positions: Vec<MeshPosition>,
    /// Stored positions to drop (undo of an automatic layout).
    #[serde(default)]
    #[ts(optional)]
    pub remove: Option<Vec<String>>,
    /// The automatic layout that placed them.
    #[serde(default)]
    #[ts(optional)]
    pub algorithm: Option<String>,
}

impl Validate for MeshPositionsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_scope(self.view, self.scope_id.as_deref().unwrap_or(""))?;
        let remove = self.remove.as_deref().unwrap_or_default();
        if self.positions.len() > MAX_POSITIONS || remove.len() > MAX_POSITIONS {
            return Err(CommandError::invalid_input("positions", "too many"));
        }
        for position in &self.positions {
            check_id("positions", &position.node_id)?;
            if !position.x.is_finite()
                || !position.y.is_finite()
                || position.x.abs() > 1e7
                || position.y.abs() > 1e7
            {
                return Err(CommandError::invalid_input("positions", "out of range"));
            }
        }
        for node in remove {
            check_id("remove", node)?;
        }
        if self
            .algorithm
            .as_ref()
            .is_some_and(|name| name.is_empty() || name.len() > 100)
        {
            return Err(CommandError::invalid_input(
                "algorithm",
                "1 to 100 characters",
            ));
        }
        Ok(())
    }
}

/// Saves where nodes of a view are.
#[tauri::command]
pub async fn mesh_save_positions(
    state: State<'_, AppState>,
    request: Input<MeshPositionsRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_mesh(|mesh, _| async move {
            mesh.save_positions(
                request.view,
                request.scope_id.as_deref().unwrap_or(""),
                &request.positions,
                request.remove.as_deref().unwrap_or_default(),
                request.algorithm.as_deref(),
            )
            .await
            .map_err(mesh_error)
        })
        .await
}

/// Request of `mesh_suggestions`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshSuggestionsRequest {
    /// 1 to 500; 100 if missing.
    #[serde(default)]
    #[ts(optional)]
    pub limit: Option<u32>,
}

impl Validate for MeshSuggestionsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.limit.is_some_and(|limit| !(1..=500).contains(&limit)) {
            return Err(CommandError::invalid_input("limit", "1 to 500"));
        }
        Ok(())
    }
}

/// Open suggestions with the threshold they were made with.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeshSuggestionList {
    pub threshold: f64,
    pub suggestions: Vec<MeshSuggestion>,
}

/// Open suggestions, the best first.
#[tauri::command]
pub async fn mesh_suggestions(
    state: State<'_, AppState>,
    request: Input<MeshSuggestionsRequest>,
) -> Result<MeshSuggestionList, CommandError> {
    let limit = request.into_inner().limit.unwrap_or(100);
    let mesh = state.session.mesh().await?;
    Ok(MeshSuggestionList {
        threshold: mesh.threshold().await,
        suggestions: mesh.suggestions(limit).await.map_err(mesh_error)?,
    })
}

/// Runs the grouping analysis in the background; cancellable.
#[tauri::command]
pub async fn mesh_analyze(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
    on_progress: Channel<AnalysisProgress>,
) -> Result<AnalysisReport, CommandError> {
    let _ = request.into_inner();
    let mesh = state.session.mesh().await?;
    let threshold = mesh.threshold().await;
    let last = std::sync::Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let progress = std::sync::Arc::new(move |progress: AnalysisProgress| {
        let mut last = last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last.elapsed() >= std::time::Duration::from_millis(100) {
            *last = std::time::Instant::now();
            let _ = on_progress.send(progress);
        }
    });
    mesh.analyze(threshold, progress).await.map_err(mesh_error)
}

/// Stops the running analysis; `false` if none runs.
#[tauri::command]
pub async fn mesh_analysis_cancel(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<bool, CommandError> {
    let _ = request.into_inner();
    Ok(state.session.mesh().await?.cancel_analysis())
}

/// Request of `mesh_suggestion_decide`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshDecisionRequest {
    pub id: String,
    pub decision: SuggestionDecision,
}

impl Validate for MeshDecisionRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)
    }
}

/// "Annehmen" (a connection of kind `similarity`) or "Ablehnen".
#[tauri::command]
pub async fn mesh_suggestion_decide(
    state: State<'_, AppState>,
    request: Input<MeshDecisionRequest>,
) -> Result<Option<MeshConnection>, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_mesh(|mesh, _| async move {
            mesh.decide(&request.id, request.decision)
                .await
                .map_err(mesh_error)
        })
        .await
}

/// Request of `mesh_set_threshold`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshThresholdRequest {
    pub threshold: f64,
}

impl Validate for MeshThresholdRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.threshold.is_finite() && (0.05..=0.95).contains(&self.threshold) {
            Ok(())
        } else {
            Err(CommandError::invalid_input("threshold", "0.05 to 0.95"))
        }
    }
}

/// Changes the threshold of the analysis (a library setting).
#[tauri::command]
pub async fn mesh_set_threshold(
    state: State<'_, AppState>,
    request: Input<MeshThresholdRequest>,
) -> Result<f64, CommandError> {
    let threshold = request.into_inner().threshold;
    let mesh = state.session.mesh().await?;
    if mesh.set_threshold(threshold).await {
        Ok(threshold)
    } else {
        Err(CommandError::new(ErrorCode::SettingsNotSaved))
    }
}
