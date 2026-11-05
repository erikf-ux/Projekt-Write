//! Subnets, memberships, tags and the structural undo (Phase 7, ADR 0017).
//!
//! The acceptance criteria of Phase 7 in Rust: cycles are refused; a
//! document in two subnets stays one row and one file; deleting a subnet
//! deletes no element. Every step can be undone and redone.

mod common;

use std::sync::Arc;

use project_write_lib::clock::{Generators, ManualClock, RandomIds, Timestamp};
use project_write_lib::database::structure::{SubnetRow, TagRow};
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::library::Library;
use project_write_lib::services::pwrite;
use project_write_lib::services::structure::{
    DocumentStructure, Structure, StructureAction, StructureError,
};
use uuid::Uuid;

const MINUTE: i64 = 60 * 1000;

struct Fixture {
    _dir: tempfile::TempDir,
    clock: Arc<ManualClock>,
    library: Library,
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-structure-");
    let clock = Arc::new(ManualClock::new(
        Timestamp::parse_iso("2026-11-05T09:00:00.000Z").unwrap(),
    ));
    let generators = Generators::new(clock.clone(), Arc::new(RandomIds));
    let (library, _) = Library::create(&dir.path().join("lib"), generators)
        .await
        .unwrap();
    Fixture {
        _dir: dir,
        clock,
        library,
    }
}

fn uuid(id: Option<&str>) -> Uuid {
    Uuid::parse_str(id.expect("an id")).unwrap()
}

async fn subnet(structure: &Structure, name: &str, parent: Option<Uuid>) -> Uuid {
    let outcome = structure
        .create_subnet(name, parent, None, "")
        .await
        .unwrap();
    uuid(outcome.subject_id.as_deref())
}

/// Everything the structure holds, for comparing states.
#[derive(Debug, PartialEq, Eq)]
struct State {
    subnets: Vec<SubnetRow>,
    tags: Vec<TagRow>,
    documents: Vec<DocumentStructure>,
    trash: Vec<String>,
    archive: Vec<String>,
}

async fn state(f: &Fixture) -> State {
    let structure = f.library.structure();
    let documents = f.library.documents();
    let mut trash = Vec::new();
    let mut archive = Vec::new();
    for meta in documents.list().await.unwrap() {
        if meta.archived_at.is_some() {
            archive.push(meta.id.to_string());
        }
    }
    for meta in documents.list_trash().await.unwrap() {
        trash.push(meta.id.to_string());
    }
    State {
        subnets: structure.subnets().await.unwrap(),
        tags: structure.tags().await.unwrap(),
        documents: structure.all_documents().await.unwrap(),
        trash,
        archive,
    }
}

fn names(rows: &[SubnetRow]) -> Vec<(String, Option<String>)> {
    let by_id = |id: &Option<String>| {
        id.as_ref()
            .map(|id| rows.iter().find(|row| &row.id == id).unwrap().name.clone())
    };
    let mut list: Vec<(String, Option<String>)> = rows
        .iter()
        .map(|row| (row.name.clone(), by_id(&row.parent_id)))
        .collect();
    list.sort();
    list
}

#[tokio::test]
async fn cycles_are_refused_by_rust_and_by_the_database() {
    let f = fixture().await;
    let structure = f.library.structure();
    let projects = subnet(&structure, "Projekte", None).await;
    let novel = subnet(&structure, "Roman", Some(projects)).await;
    let chapter = subnet(&structure, "Kapitel", Some(novel)).await;

    for (id, parent) in [(projects, chapter), (projects, novel), (novel, novel)] {
        let error = structure.move_subnet(id, Some(parent)).await.unwrap_err();
        assert!(matches!(error, StructureError::Cycle), "{error:?}");
    }
    // The trigger refuses it even without Rust.
    let error = sqlx::query("UPDATE subnets SET parent_id = ?1 WHERE id = ?2")
        .bind(chapter.to_string())
        .bind(projects.to_string())
        .execute(f.library.database().writer())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("subnet cycle"), "{error}");
    // A legal move works, and the tree is unchanged by the refusals.
    structure
        .move_subnet(chapter, Some(projects))
        .await
        .unwrap();
    assert_eq!(
        names(&structure.subnets().await.unwrap()),
        [
            ("Kapitel".to_owned(), Some("Projekte".to_owned())),
            ("Projekte".to_owned(), None),
            ("Roman".to_owned(), Some("Projekte".to_owned())),
        ]
    );
    f.library.close().await;
}

#[tokio::test]
async fn a_document_in_two_subnets_stays_one_row_and_one_file() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let a = subnet(&structure, "Recherche", None).await;
    let b = subnet(&structure, "Kapitel", None).await;
    let doc = documents
        .create("Quellen", DocumentFormat::Md, b"Text\n")
        .await
        .unwrap();
    structure.assign(doc.id, a).await.unwrap();
    f.clock.advance(MINUTE);
    structure.assign(doc.id, b).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(f.library.database().reader())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let files = std::fs::read_dir(f.library.root().path().join("documents/markdown"))
        .unwrap()
        .count();
    assert_eq!(files, 1);
    // The first membership is primary.
    let entry = structure.document(&doc.id.to_string()).await.unwrap();
    assert_eq!(entry.subnet_ids, [a.to_string(), b.to_string()]);
    // A second assignment to the same subnet changes nothing.
    let outcome = structure.assign(doc.id, a).await.unwrap();
    assert!(outcome.step.is_none());
    f.library.close().await;
}

#[tokio::test]
async fn deleting_a_subnet_deletes_no_element() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let projects = subnet(&structure, "Projekte", None).await;
    let novel = subnet(&structure, "Roman", Some(projects)).await;
    let notes = subnet(&structure, "Notizen", Some(projects)).await;
    // A subnet with the same name waits one level up.
    let other_notes = subnet(&structure, "Notizen", None).await;
    let both = documents
        .create("Beide", DocumentFormat::Md, b"")
        .await
        .unwrap();
    let only = documents
        .create("Nur Projekte", DocumentFormat::Md, b"")
        .await
        .unwrap();
    structure.assign(both.id, projects).await.unwrap();
    f.clock.advance(MINUTE);
    structure.assign(both.id, other_notes).await.unwrap();
    structure.assign(only.id, projects).await.unwrap();

    structure.delete_subnet(projects, false).await.unwrap();
    assert_eq!(documents.list().await.unwrap().len(), 2);
    // The subnets moved up; the taken name got a number.
    assert_eq!(
        names(&structure.subnets().await.unwrap()),
        [
            ("Notizen".to_owned(), None),
            ("Notizen (2)".to_owned(), None),
            ("Roman".to_owned(), None),
        ]
    );
    let _ = (novel, notes);
    // The remaining membership became primary; the other document is
    // "Nicht zugeordnet".
    assert_eq!(
        structure
            .document(&both.id.to_string())
            .await
            .unwrap()
            .subnet_ids,
        [other_notes.to_string()]
    );
    assert!(
        structure
            .document(&only.id.to_string())
            .await
            .unwrap()
            .subnet_ids
            .is_empty()
    );

    // With its subnets: still no document goes.
    let parent = subnet(&structure, "Archivkisten", None).await;
    let child = subnet(&structure, "Kiste 1", Some(parent)).await;
    structure.assign(only.id, child).await.unwrap();
    structure.delete_subnet(parent, true).await.unwrap();
    assert_eq!(documents.list().await.unwrap().len(), 2);
    assert!(
        !structure
            .subnets()
            .await
            .unwrap()
            .iter()
            .any(|row| row.name.starts_with("Kiste"))
    );
    f.library.close().await;
}

#[tokio::test]
async fn moving_changes_the_primary_subnet_and_keeps_the_others() {
    let f = fixture().await;
    let structure = f.library.structure();
    let doc = f
        .library
        .documents()
        .create("Szene", DocumentFormat::Md, b"")
        .await
        .unwrap();
    let a = subnet(&structure, "A", None).await;
    let b = subnet(&structure, "B", None).await;
    let c = subnet(&structure, "C", None).await;
    let id = doc.id.to_string();
    let ids = |list: &[Uuid]| list.iter().map(Uuid::to_string).collect::<Vec<_>>();

    structure.move_document(doc.id, Some(a)).await.unwrap();
    f.clock.advance(MINUTE);
    structure.assign(doc.id, b).await.unwrap();
    assert_eq!(
        structure.document(&id).await.unwrap().subnet_ids,
        ids(&[a, b])
    );
    // Moving to C: A (primary) goes, B stays, C is primary.
    f.clock.advance(MINUTE);
    structure.move_document(doc.id, Some(c)).await.unwrap();
    assert_eq!(
        structure.document(&id).await.unwrap().subnet_ids,
        ids(&[c, b])
    );
    // Moving to B, where it is already: B becomes primary, C goes.
    structure.move_document(doc.id, Some(b)).await.unwrap();
    assert_eq!(structure.document(&id).await.unwrap().subnet_ids, ids(&[b]));
    // Primary by choice, and the oldest remaining one moves up.
    f.clock.advance(MINUTE);
    structure.assign(doc.id, a).await.unwrap();
    structure.set_primary(doc.id, a).await.unwrap();
    assert_eq!(
        structure.document(&id).await.unwrap().subnet_ids,
        ids(&[a, b])
    );
    structure.unassign(doc.id, a).await.unwrap();
    assert_eq!(structure.document(&id).await.unwrap().subnet_ids, ids(&[b]));
    assert!(matches!(
        structure.unassign(doc.id, c).await,
        Err(StructureError::NotMember)
    ));
    // Without a target: "Nicht zugeordnet".
    structure.move_document(doc.id, None).await.unwrap();
    assert!(structure.document(&id).await.unwrap().subnet_ids.is_empty());
    f.library.close().await;
}

#[tokio::test]
async fn tags_are_unique_without_case_and_can_be_merged() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let first = documents
        .create("Eins", DocumentFormat::Md, b"")
        .await
        .unwrap();
    let second = documents
        .create("Zwei", DocumentFormat::Md, b"")
        .await
        .unwrap();

    let apples = uuid(
        structure
            .tag_document(first.id, "Äpfel")
            .await
            .unwrap()
            .subject_id
            .as_deref(),
    );
    // Same tag in another spelling.
    let again = structure.tag_document(second.id, "äPFEL").await.unwrap();
    assert_eq!(uuid(again.subject_id.as_deref()), apples);
    assert!(matches!(
        structure.create_tag("ÄPFEL").await,
        Err(StructureError::TagNameTaken { existing }) if existing == apples.to_string()
    ));
    let fruit = uuid(
        structure
            .create_tag("Obst")
            .await
            .unwrap()
            .subject_id
            .as_deref(),
    );
    structure.tag_document(first.id, "Obst").await.unwrap();
    // Renaming onto another tag's name is refused with that tag.
    assert!(matches!(
        structure.rename_tag(fruit, "äpfel").await,
        Err(StructureError::TagNameTaken { existing }) if existing == apples.to_string()
    ));
    // Case only: allowed.
    structure.rename_tag(apples, "äpfel").await.unwrap();

    // Merging: "Eins" had both, keeps one.
    structure.merge_tags(apples, fruit).await.unwrap();
    let tags = structure.tags().await.unwrap();
    assert_eq!(
        tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["Obst"]
    );
    assert_eq!(
        structure
            .document(&first.id.to_string())
            .await
            .unwrap()
            .tag_ids,
        [fruit.to_string()]
    );
    assert_eq!(
        structure
            .document(&second.id.to_string())
            .await
            .unwrap()
            .tag_ids,
        [fruit.to_string()]
    );
    // Deleting a tag keeps the documents.
    structure.delete_tag(fruit).await.unwrap();
    assert_eq!(documents.list().await.unwrap().len(), 2);
    assert!(structure.all_documents().await.unwrap().is_empty());
    f.library.close().await;
}

#[tokio::test]
async fn colours_follow_the_parent_or_go_round_the_cluster_colours() {
    let f = fixture().await;
    let structure = f.library.structure();
    let mut colors = Vec::new();
    for name in ["Eins", "Zwei", "Drei", "Vier", "Fünf", "Sechs"] {
        subnet(&structure, name, None).await;
        colors.push(
            structure
                .subnets()
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.name == name)
                .unwrap()
                .color,
        );
    }
    assert_eq!(
        colors,
        ["blue", "green", "violet", "orange", "grey", "blue"]
    );
    let parent = structure
        .create_subnet("Eigen", None, Some("#D9480F"), "Rost")
        .await
        .unwrap();
    let child = subnet(&structure, "Kind", Some(uuid(parent.subject_id.as_deref()))).await;
    let rows = structure.subnets().await.unwrap();
    let child = rows.iter().find(|row| row.id == child.to_string()).unwrap();
    assert_eq!(child.color, "#d9480f");
    assert!(matches!(
        structure
            .create_subnet("Hell", None, Some("#fafafa"), "")
            .await,
        Err(StructureError::InvalidColor("contrast"))
    ));
    // Siblings never share a name.
    assert!(matches!(
        structure.create_subnet("eins", None, None, "").await,
        Err(StructureError::SubnetNameTaken { .. })
    ));
    f.library.close().await;
}

#[tokio::test]
async fn every_step_is_undone_and_redone_in_order() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let doc = documents
        .create("Kapitel 1", DocumentFormat::Md, b"")
        .await
        .unwrap();
    let start = state(&f).await;

    // A sequence that passes through a swap of parents and names.
    let a = subnet(&structure, "A", None).await;
    let b = subnet(&structure, "B", Some(a)).await;
    structure.move_subnet(b, None).await.unwrap();
    structure.move_subnet(a, Some(b)).await.unwrap();
    structure
        .update_subnet(a, Some("Neu"), Some("Beschreibung"), Some("green"))
        .await
        .unwrap();
    f.clock.advance(MINUTE);
    structure.assign(doc.id, a).await.unwrap();
    f.clock.advance(MINUTE);
    structure.assign(doc.id, b).await.unwrap();
    structure.move_document(doc.id, Some(b)).await.unwrap();
    let tag = uuid(
        structure
            .tag_document(doc.id, "Entwurf")
            .await
            .unwrap()
            .subject_id
            .as_deref(),
    );
    let other = uuid(
        structure
            .create_tag("Fassung")
            .await
            .unwrap()
            .subject_id
            .as_deref(),
    );
    structure.rename_tag(tag, "Skizze").await.unwrap();
    structure.rename_tag(other, "Entwurf").await.unwrap();
    structure.merge_tags(tag, other).await.unwrap();
    structure.set_archived(doc.id, true).await.unwrap();
    structure.set_deleted(doc.id, true).await.unwrap();
    structure.delete_subnet(b, false).await.unwrap();
    let end = state(&f).await;
    assert_eq!(
        structure.history().undo.map(|step| step.action),
        Some(StructureAction::SubnetDelete)
    );

    let mut steps = 0;
    while structure.undo().await.unwrap().is_some() {
        steps += 1;
    }
    assert_eq!(steps, 16);
    assert_eq!(state(&f).await, start);
    while structure.redo().await.unwrap().is_some() {}
    assert_eq!(state(&f).await, end);
    // Half way back and forth.
    for _ in 0..8 {
        structure.undo().await.unwrap();
    }
    let middle = state(&f).await;
    structure.undo().await.unwrap();
    structure.redo().await.unwrap();
    assert_eq!(state(&f).await, middle);
    // A new step drops what could be redone.
    subnet(&structure, "Neu dazu", None).await;
    assert!(structure.history().redo.is_none());
    f.library.close().await;
}

#[tokio::test]
async fn a_step_that_no_longer_fits_is_refused() {
    let f = fixture().await;
    let structure = f.library.structure();
    let a = subnet(&structure, "A", None).await;
    // Changed behind the journal's back.
    sqlx::query("UPDATE subnets SET description = 'anders' WHERE id = ?1")
        .bind(a.to_string())
        .execute(f.library.database().writer())
        .await
        .unwrap();
    assert!(matches!(
        structure.undo().await,
        Err(StructureError::Changed)
    ));
    assert_eq!(structure.history().undo, None);
    assert_eq!(structure.subnets().await.unwrap().len(), 1);
    f.library.close().await;
}

#[tokio::test]
async fn trash_copy_and_purge_treat_memberships_and_tags_right() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let a = subnet(&structure, "A", None).await;
    let b = subnet(&structure, "B", None).await;
    let doc = documents
        .create("Original", DocumentFormat::Md, b"x\n")
        .await
        .unwrap();
    structure.assign(doc.id, a).await.unwrap();
    f.clock.advance(MINUTE);
    structure.assign(doc.id, b).await.unwrap();
    structure.set_primary(doc.id, b).await.unwrap();
    structure.tag_document(doc.id, "Wichtig").await.unwrap();
    let original = structure.document(&doc.id.to_string()).await.unwrap();

    // Section 13: the copy keeps tags and subnets (same primary).
    let copy = documents.copy(doc.id, "Kopie").await.unwrap();
    structure.copy_to(doc.id, copy.id).await.unwrap();
    let copied = structure.document(&copy.id.to_string()).await.unwrap();
    assert_eq!(copied.subnet_ids, original.subnet_ids);
    assert_eq!(copied.tag_ids, original.tag_ids);

    // Stage 3: in the trash and back, with everything.
    structure.set_deleted(doc.id, true).await.unwrap();
    assert_eq!(
        structure.document(&doc.id.to_string()).await.unwrap(),
        original
    );
    structure.set_deleted(doc.id, false).await.unwrap();
    assert_eq!(
        structure.document(&doc.id.to_string()).await.unwrap(),
        original
    );
    assert!(matches!(
        structure.set_deleted(doc.id, false).await,
        Err(StructureError::NotInTrash)
    ));
    // Removed for good: the rows go with the document; undo is refused.
    structure.set_deleted(doc.id, true).await.unwrap();
    documents.purge(doc.id).await.unwrap();
    let left = structure.all_documents().await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, copy.id.to_string());
    assert!(matches!(
        structure.undo().await,
        Err(StructureError::Changed)
    ));
    f.library.close().await;
}

#[tokio::test]
async fn pwrite_files_carry_their_tags() {
    let f = fixture().await;
    let structure = f.library.structure();
    let documents = f.library.documents();
    let doc = documents
        .create_pwrite("Roman", "de-DE", pwrite::empty_content())
        .await
        .unwrap();
    structure.tag_document(doc.id, "Krimi").await.unwrap();
    structure.tag_document(doc.id, "Entwurf").await.unwrap();
    documents
        .save_pwrite(
            doc.id,
            serde_json::json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hallo" }] }] }),
        )
        .await
        .unwrap();
    let (_, data) = documents.read(doc.id).await.unwrap();
    let pwrite::Parsed::Current(file) = pwrite::parse(&data).unwrap() else {
        panic!("not version 1");
    };
    assert_eq!(file.tags, ["Entwurf", "Krimi"]);

    // An imported .pwrite brings its tags (the command does this step).
    let imported = documents
        .import("Kopie", DocumentFormat::Pwrite, &data, "kopie.pwrite")
        .await
        .unwrap();
    structure
        .tag_by_names(imported.id, &file.tags)
        .await
        .unwrap();
    let tags = structure
        .document(&imported.id.to_string())
        .await
        .unwrap()
        .tag_ids;
    assert_eq!(tags.len(), 2);
    assert_eq!(structure.tags().await.unwrap().len(), 2);
    f.library.close().await;
}
