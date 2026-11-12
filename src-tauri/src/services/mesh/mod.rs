//! The Mesh of one open library (section 15, ADR 0022): the views, the
//! confirmed connections and the positions of each view. Suggestions of
//! the grouping analysis are in [`suggestions`].

pub mod graph;
pub mod suggestions;

use std::collections::BTreeSet;
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

/// The Mesh of one open library.
#[derive(Debug, Clone)]
pub struct Mesh {
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
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
    ) -> Self {
        Self {
            database,
            generators,
            write_lock,
        }
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
