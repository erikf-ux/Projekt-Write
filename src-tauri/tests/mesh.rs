//! The Mesh with a real library (section 15, ADR 0022): nodes come and go
//! with their elements, views follow subnets and links, connections are
//! drawn, removed and restored, positions are stored per view, and derived
//! connections never land in `mesh_connections`.

mod common;

use std::collections::BTreeSet;

use project_write_lib::clock::Generators;
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::library::Library;
use project_write_lib::services::mesh::{
    GraphQuery, MeshEdgeKind, MeshError, MeshNodeKind, MeshPosition, MeshView, UNASSIGNED_ID,
};
use uuid::Uuid;

struct Fixture {
    _dir: tempfile::TempDir,
    library: Library,
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-mesh-");
    let (library, _) = Library::create(&dir.path().join("lib"), Generators::system())
        .await
        .unwrap();
    Fixture { _dir: dir, library }
}

fn query(view: MeshView, scope: &str, expanded: &[&str]) -> GraphQuery {
    GraphQuery {
        view,
        scope_id: scope.to_owned(),
        expanded: expanded.iter().map(|s| (*s).to_owned()).collect(),
        depth: 1,
    }
}

async fn count(library: &Library, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(library.database().reader())
        .await
        .unwrap()
}

/// Subnet "Projekte" with "Brücke" in it; "Hafen" in Projekte and also in
/// "Ideen"; "Notiz" without a subnet links to "Brücke".
struct Library3 {
    projekte: String,
    ideen: String,
    bruecke: String,
    hafen: String,
    notiz: String,
}

async fn three(f: &Fixture) -> Library3 {
    let structure = f.library.structure();
    let documents = f.library.documents();
    let projekte = structure
        .create_subnet("Projekte", None, None, "")
        .await
        .unwrap()
        .subject_id
        .unwrap();
    let ideen = structure
        .create_subnet("Ideen", None, None, "")
        .await
        .unwrap()
        .subject_id
        .unwrap();
    let bruecke = documents
        .create("Brücke", DocumentFormat::Md, b"Stahl")
        .await
        .unwrap()
        .id;
    let hafen = documents
        .create("Hafen", DocumentFormat::Md, b"Kai")
        .await
        .unwrap()
        .id;
    let notiz = documents
        .create(
            "Notiz",
            DocumentFormat::Md,
            format!("Siehe [Brücke](pwrite://{bruecke}).").as_bytes(),
        )
        .await
        .unwrap()
        .id;
    let projekte_id = Uuid::parse_str(&projekte).unwrap();
    let ideen_id = Uuid::parse_str(&ideen).unwrap();
    structure.assign(bruecke, projekte_id).await.unwrap();
    structure.assign(hafen, projekte_id).await.unwrap();
    structure.assign(hafen, ideen_id).await.unwrap();
    Library3 {
        projekte,
        ideen,
        bruecke: bruecke.to_string(),
        hafen: hafen.to_string(),
        notiz: notiz.to_string(),
    }
}

#[tokio::test]
async fn every_element_and_subnet_has_a_node_until_it_is_gone() {
    let f = fixture().await;
    let l = three(&f).await;
    assert_eq!(
        count(&f.library, "SELECT count(*) FROM mesh_nodes").await,
        5
    );
    let mesh = f.library.mesh();
    mesh.connect(&l.notiz, &l.hafen, None).await.unwrap();
    // Out of the trash for good: node and connection go.
    let documents = f.library.documents();
    let notiz = Uuid::parse_str(&l.notiz).unwrap();
    documents.move_to_trash(notiz).await.unwrap();
    documents.purge(notiz).await.unwrap();
    assert_eq!(
        count(&f.library, "SELECT count(*) FROM mesh_nodes").await,
        4
    );
    assert!(mesh.connections().await.unwrap().is_empty());
    f.library.close().await;
}

#[tokio::test]
async fn the_views_follow_subnets_links_and_connections() {
    let f = fixture().await;
    let l = three(&f).await;
    let mesh = f.library.mesh();
    let manual = mesh
        .connect(&l.bruecke, &l.ideen, Some("Thema"))
        .await
        .unwrap();

    // Global View: two closed clusters and the unassigned one.
    let graph = mesh.graph(&query(MeshView::Global, "", &[])).await.unwrap();
    let kinds: Vec<_> = graph.nodes.iter().map(|n| (n.id.clone(), n.kind)).collect();
    assert_eq!(
        kinds,
        vec![
            (l.ideen.clone(), MeshNodeKind::Subnet),
            (l.projekte.clone(), MeshNodeKind::Subnet),
            (UNASSIGNED_ID.to_owned(), MeshNodeKind::Unassigned)
        ]
    );
    let projekte = graph.nodes.iter().find(|n| n.id == l.projekte).unwrap();
    assert_eq!(projekte.count, 2);
    let edges: BTreeSet<_> = graph
        .edges
        .iter()
        .map(|e| (e.kind, e.source.clone(), e.target.clone(), e.count))
        .collect();
    let (a, b) = if l.ideen < l.projekte {
        (l.ideen.clone(), l.projekte.clone())
    } else {
        (l.projekte.clone(), l.ideen.clone())
    };
    assert!(edges.contains(&(
        MeshEdgeKind::Reference,
        UNASSIGNED_ID.to_owned(),
        l.projekte.clone(),
        1
    )));
    // Brücke–Ideen and Hafen in Ideen: both between the two clusters.
    assert!(edges.contains(&(MeshEdgeKind::Manual, a.clone(), b.clone(), 1)));
    assert!(edges.contains(&(MeshEdgeKind::Membership, a, b, 1)));

    // Opened: the documents in the hull of Projekte, the connection itself.
    let graph = mesh
        .graph(&query(MeshView::Global, "", &[&l.projekte, UNASSIGNED_ID]))
        .await
        .unwrap();
    let bruecke = graph.nodes.iter().find(|n| n.id == l.bruecke).unwrap();
    assert_eq!(bruecke.parent_id.as_deref(), Some(l.projekte.as_str()));
    let edge = graph
        .edges
        .iter()
        .find(|e| e.kind == MeshEdgeKind::Manual)
        .unwrap();
    assert_eq!(edge.connection_id.as_deref(), Some(manual.id.as_str()));
    assert!(graph.edges.iter().any(|e| e.kind == MeshEdgeKind::Reference
        && e.source == l.notiz
        && e.target == l.bruecke
        && e.directed));

    // Focus View of Hafen: its subnets.
    let graph = mesh
        .graph(&query(MeshView::Focus, &l.hafen, &[]))
        .await
        .unwrap();
    let ids: BTreeSet<_> = graph.nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(
        ids,
        [l.hafen.clone(), l.projekte.clone(), l.ideen.clone()]
            .into_iter()
            .collect()
    );
    f.library.close().await;
}

#[tokio::test]
async fn connections_are_drawn_removed_and_restored_and_derived_ones_are_not_stored() {
    let f = fixture().await;
    let l = three(&f).await;
    let mesh = f.library.mesh();
    assert!(matches!(
        mesh.connect(&l.hafen, &l.hafen, None).await,
        Err(MeshError::SameNode)
    ));
    assert!(matches!(
        mesh.connect(&l.hafen, &Uuid::new_v4().to_string(), None)
            .await,
        Err(MeshError::NodeNotFound)
    ));
    let made = mesh.connect(&l.hafen, &l.bruecke, None).await.unwrap();
    assert!(made.node_a < made.node_b);
    assert!(matches!(
        mesh.connect(&l.bruecke, &l.hafen, None).await,
        Err(MeshError::ConnectionExists)
    ));
    // The link Notiz → Brücke and the memberships are derived: not stored.
    assert_eq!(mesh.connections().await.unwrap(), vec![made.clone()]);

    let removed = mesh.disconnect(&made.id).await.unwrap();
    assert!(mesh.connections().await.unwrap().is_empty());
    assert!(matches!(
        mesh.disconnect(&made.id).await,
        Err(MeshError::ConnectionNotFound)
    ));
    let restored = mesh.restore(&removed).await.unwrap();
    assert_eq!(restored.id, made.id);
    assert_eq!(mesh.connections().await.unwrap().len(), 1);
    f.library.close().await;
}

#[tokio::test]
async fn positions_are_stored_per_view_and_can_be_dropped() {
    let f = fixture().await;
    let l = three(&f).await;
    let mesh = f.library.mesh();
    let at = |node: &str, x: f64, pinned: bool| MeshPosition {
        node_id: node.to_owned(),
        x,
        y: x / 2.0,
        pinned,
    };
    mesh.save_positions(
        MeshView::Global,
        "",
        &[at(&l.projekte, 100.0, true), at(&l.ideen, -40.0, false)],
        &[],
        Some("elk.layered"),
    )
    .await
    .unwrap();
    mesh.save_positions(
        MeshView::Subnet,
        &l.projekte,
        &[at(&l.hafen, 7.0, true)],
        &[],
        None,
    )
    .await
    .unwrap();
    let graph = mesh.graph(&query(MeshView::Global, "", &[])).await.unwrap();
    let projekte = graph.nodes.iter().find(|n| n.id == l.projekte).unwrap();
    assert_eq!(
        (projekte.x, projekte.y, projekte.pinned),
        (Some(100.0), Some(50.0), true)
    );
    // Another view keeps its own.
    let graph = mesh
        .graph(&query(MeshView::Subnet, &l.projekte, &[]))
        .await
        .unwrap();
    let hafen = graph.nodes.iter().find(|n| n.id == l.hafen).unwrap();
    assert_eq!(hafen.x, Some(7.0));
    assert_eq!(
        graph.nodes.iter().find(|n| n.id == l.projekte).unwrap().x,
        None
    );
    // Undo of a layout: dropped again.
    mesh.save_positions(
        MeshView::Global,
        "",
        &[],
        std::slice::from_ref(&l.ideen),
        None,
    )
    .await
    .unwrap();
    let graph = mesh.graph(&query(MeshView::Global, "", &[])).await.unwrap();
    assert_eq!(
        graph.nodes.iter().find(|n| n.id == l.ideen).unwrap().x,
        None
    );
    // A node that is gone is skipped, not an error.
    mesh.save_positions(
        MeshView::Global,
        "",
        &[at(&Uuid::new_v4().to_string(), 1.0, false)],
        &[],
        None,
    )
    .await
    .unwrap();
    f.library.close().await;
}

/// Every row of the tables that hold confirmed or derived relations, in a
/// fixed order: what a suggestion run must leave as it was.
async fn relations(library: &Library) -> Vec<String> {
    let mut all = Vec::new();
    for sql in [
        "SELECT id || '|' || node_a || '|' || node_b || '|' || kind || '|' || ifnull(note, '') || '|' || created_at FROM mesh_connections ORDER BY id",
        "SELECT source_id || '|' || target_id FROM document_links ORDER BY 1",
        "SELECT document_id || '|' || media_id || '|' || page FROM document_media ORDER BY 1",
        "SELECT subnet_id || '|' || coalesce(document_id, media_id) || '|' || is_primary FROM subnet_memberships ORDER BY 1",
        "SELECT document_id || '|' || tag_id FROM document_tags ORDER BY 1",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_all(library.database().reader())
            .await
            .unwrap();
        all.push(format!("{sql}: {}", rows.join(", ")));
    }
    all
}

#[tokio::test]
async fn a_suggestion_run_changes_no_confirmed_relation_and_rejected_ones_never_return() {
    use project_write_lib::services::mesh::SuggestionDecision;
    use std::sync::Arc;

    let f = fixture().await;
    let l = three(&f).await;
    let documents = f.library.documents();
    let structure = f.library.structure();
    let text = "Stahlträger und Pfeiler tragen die Brücke über den Hafen. Die Pfeiler stehen im Wasser, die Stahlträger rosten.";
    let mut similar = Vec::new();
    for title in ["Brückenbau Hafen", "Brückenbau Fluss", "Brückenbau Kanal"] {
        let id = documents
            .create(title, DocumentFormat::Md, text.as_bytes())
            .await
            .unwrap()
            .id;
        structure
            .tag_by_names(id, &["Bau".to_owned(), "Stahl".to_owned()])
            .await
            .unwrap();
        similar.push(id.to_string());
    }
    let mesh = f.library.mesh();
    mesh.connect(&l.hafen, &l.notiz, Some("von Hand"))
        .await
        .unwrap();
    let before = relations(&f.library).await;

    let report = mesh
        .analyze(mesh.threshold().await, Arc::new(|_| {}))
        .await
        .unwrap();
    assert!(report.created >= 3, "{report:?}");
    // Data comparison: every confirmed and derived relation is as it was.
    assert_eq!(relations(&f.library).await, before);
    let open = mesh.suggestions(100).await.unwrap();
    let pair = |a: &str, b: &str| {
        if a < b {
            (a.to_owned(), b.to_owned())
        } else {
            (b.to_owned(), a.to_owned())
        }
    };
    let wanted = pair(&similar[0], &similar[1]);
    let first = open
        .iter()
        .find(|s| (s.node_a.clone(), s.node_b.clone()) == wanted)
        .expect("the two bridges are suggested");
    assert!(first.reason.shared_tags == 2 && first.reason.content > 0.5);

    // Rejected: never again. Accepted: a connection of kind similarity.
    mesh.decide(&first.id, SuggestionDecision::Reject)
        .await
        .unwrap();
    let second = open
        .iter()
        .find(|s| (s.node_a.clone(), s.node_b.clone()) == pair(&similar[0], &similar[2]))
        .unwrap();
    let made = mesh
        .decide(&second.id, SuggestionDecision::Accept)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(made.kind, MeshEdgeKind::Similarity);
    assert!(matches!(
        mesh.decide(&first.id, SuggestionDecision::Accept).await,
        Err(MeshError::SuggestionNotFound)
    ));
    let before = relations(&f.library).await;
    let again = mesh
        .analyze(mesh.threshold().await, Arc::new(|_| {}))
        .await
        .unwrap();
    assert_eq!(again.created, 0, "{again:?}");
    assert_eq!(relations(&f.library).await, before);
    let statuses = mesh.all_suggestions().await.unwrap();
    assert_eq!(
        statuses
            .iter()
            .filter(|(a, b, _)| (a.clone(), b.clone()) == wanted)
            .map(|(_, _, status)| status.as_str())
            .collect::<Vec<_>>(),
        vec!["rejected"]
    );
    // Suggestions only in mesh_suggestions: one accepted connection besides
    // the manual one, nothing else.
    let connections = mesh.connections().await.unwrap();
    assert_eq!(connections.len(), 2);
    f.library.close().await;
}

#[tokio::test]
async fn only_one_analysis_runs_and_a_cancelled_one_stores_nothing() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let f = fixture().await;
    let _ = three(&f).await;
    let mesh = f.library.mesh();
    // Cancelled from its first progress report.
    let other = mesh.clone();
    let asked = Arc::new(AtomicBool::new(false));
    let flag = asked.clone();
    let result = mesh
        .analyze(
            0.05,
            Arc::new(move |_| {
                if !flag.swap(true, Ordering::Relaxed) {
                    assert!(other.cancel_analysis());
                }
            }),
        )
        .await;
    assert!(matches!(result, Err(MeshError::Cancelled)), "{result:?}");
    assert!(mesh.all_suggestions().await.unwrap().is_empty());
    assert!(!mesh.cancel_analysis());
    f.library.close().await;
}
