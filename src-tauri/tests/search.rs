//! The global search (Phase 9, section 19, ADR 0020).
//!
//! Acceptance criteria of Phase 9 in Rust: `Muller` and `Mueller` find
//! `Müller`, `Strasse` finds `Straße`, `haus` finds `Krankenhaus` and
//! `Häuser`, `C++` and a lone `"` raise no error; archived elements appear
//! only through the filter; the index is right after editing, deleting and
//! restoring.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use project_write_lib::clock::Generators;
use project_write_lib::database::search as rows;
use project_write_lib::services::documents::{DocumentFormat, DocumentMeta};
use project_write_lib::services::library::{Library, OpenOptions};
use project_write_lib::services::media::{Decision, SubnetPlacement};
use project_write_lib::services::pwrite;
use project_write_lib::services::search::{SearchHit, SearchKind, SearchMatch, SearchOptions};
use project_write_lib::services::structure::ElementKind;
use uuid::Uuid;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#3a7"/></svg>"##;

fn options() -> SearchOptions {
    SearchOptions {
        limit: 50,
        ..SearchOptions::default()
    }
}

async fn create(path: &Path) -> Library {
    Library::create(path, Generators::system()).await.unwrap().0
}

async fn open(path: &Path) -> Library {
    Library::open(path, OpenOptions::default(), Generators::system())
        .await
        .unwrap()
        .0
}

async fn document(
    library: &Library,
    title: &str,
    format: DocumentFormat,
    text: &str,
) -> DocumentMeta {
    let content = match format {
        DocumentFormat::Pwrite => pwrite::to_bytes(&pwrite::new_file(
            &Uuid::nil().to_string(),
            title,
            "2026-11-11T10:00:00.000Z",
            "de",
            serde_json::json!({
                "type": "doc",
                "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
            }),
        )),
        _ => text.as_bytes().to_vec(),
    };
    library
        .documents()
        .create(title, format, &content)
        .await
        .unwrap()
}

async fn search(library: &Library, query: &str) -> Vec<SearchHit> {
    search_with(library, query, &options()).await
}

async fn search_with(library: &Library, query: &str, options: &SearchOptions) -> Vec<SearchHit> {
    library.search().search(query, options).await.unwrap()
}

fn ids(hits: &[SearchHit]) -> Vec<String> {
    hits.iter().map(|hit| hit.id.clone()).collect()
}

fn found(hits: &[SearchHit], meta: &DocumentMeta) -> bool {
    hits.iter().any(|hit| hit.id == meta.id.to_string())
}

async fn import_svg(library: &Library, name: &str) -> Uuid {
    let media = library.media();
    let preview = media.prepare_bytes(name, SVG.as_bytes()).await.unwrap();
    let outcome = media
        .commit_import(
            Uuid::try_parse(&preview.batch_id).unwrap(),
            &BTreeMap::new(),
            Decision::Import,
            &SubnetPlacement::default(),
        )
        .await
        .unwrap();
    Uuid::try_parse(&outcome.items[0].id).unwrap()
}

/// Entries for every document, medium and subnet, and as many FTS rows.
async fn assert_consistent(library: &Library) {
    let pool = library.database().reader();
    let elements: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM documents) + (SELECT count(*) FROM media) + (SELECT count(*) FROM subnets)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let (entries, words, parts) = rows::counts(pool).await.unwrap();
    assert_eq!((entries, words, parts), (elements, elements, elements));
    assert!(rows::stale_documents(pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn spellings_find_umlauts_sharp_s_and_parts_of_words() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let mueller = document(
        &library,
        "Brief",
        DocumentFormat::Md,
        "Sehr geehrter Herr **Müller**,",
    )
    .await;
    let strasse = document(&library, "Adresse", DocumentFormat::Txt, "Hauptstraße 5").await;
    let spital = document(
        &library,
        "Dienstplan",
        DocumentFormat::Pwrite,
        "Schicht im Krankenhaus",
    )
    .await;
    let haeuser = document(
        &library,
        "Altstadt",
        DocumentFormat::Txt,
        "Alte Häuser am Markt",
    )
    .await;
    let other = document(&library, "Einkauf", DocumentFormat::Txt, "Milch und Brot").await;

    for query in ["Muller", "Mueller", "Müller", "MÜLLER", "müll"] {
        let hits = search(&library, query).await;
        assert_eq!(ids(&hits), [mueller.id.to_string()], "{query}");
        assert_eq!(hits[0].matched, SearchMatch::Words, "{query}");
    }

    // A whole word and a part of one: "Strasse" finds "Hauptstraße" only as
    // a part, "Hauptstrasse" as a word.
    let hits = search(&library, "Strasse").await;
    assert_eq!(ids(&hits), [strasse.id.to_string()]);
    assert_eq!(hits[0].matched, SearchMatch::Parts);
    let hits = search(&library, "Hauptstrasse").await;
    assert_eq!(ids(&hits), [strasse.id.to_string()]);
    assert_eq!(hits[0].matched, SearchMatch::Words);

    let hits = search(&library, "haus").await;
    assert!(found(&hits, &spital), "{hits:?}");
    assert!(found(&hits, &haeuser), "{hits:?}");
    assert!(!found(&hits, &other));
    // Words before parts: "Häuser" begins with the word, "Krankenhaus" only
    // contains it.
    assert_eq!(hits[0].id, haeuser.id.to_string());
    assert_eq!(hits[0].matched, SearchMatch::Words);
    assert_eq!(hits[1].id, spital.id.to_string());
    assert_eq!(hits[1].matched, SearchMatch::Parts);

    // The snippet marks what matched.
    let marked: Vec<&str> = hits[0]
        .snippet
        .iter()
        .filter(|part| part.hit)
        .map(|part| part.text.as_str())
        .collect();
    assert_eq!(marked, ["Häuser"]);
    library.close().await;
}

#[tokio::test]
async fn signs_and_quotes_raise_no_error() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let cpp = document(
        &library,
        "Sprachen",
        DocumentFormat::Md,
        "Ich programmiere in C++ und Rust.",
    )
    .await;
    document(
        &library,
        "Zitat",
        DocumentFormat::Txt,
        "Er sagte \"Hallo\" und ging.",
    )
    .await;
    document(
        &library,
        "Code",
        DocumentFormat::Txt,
        "Cobol, C und Clojure",
    )
    .await;

    for query in [
        "\"", "\"\"", "C++", "c++ rust", "\"Hallo", "Hallo\"", "a\"b", "*", "(", ")", "-", "NOT",
        "AND OR", "NEAR(", "^", ":", "title:x", "{x}", "'", "\\", "%", "_",
    ] {
        library
            .search()
            .search(query, &options())
            .await
            .unwrap_or_else(|error| panic!("{query}: {error}"));
    }
    assert!(search(&library, "\"").await.is_empty());
    // "C++" is neither the beginning of every word with a c nor the word "C".
    let hits = search(&library, "C++").await;
    assert_eq!(ids(&hits), [cpp.id.to_string()]);
    assert_eq!(hits[0].matched, SearchMatch::Words);
    let hits = search(&library, "\"Hallo\"").await;
    assert_eq!(hits.len(), 1);
    library.close().await;
}

#[tokio::test]
async fn archived_and_deleted_elements_appear_only_through_the_filter() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let documents = library.documents();
    let active = document(
        &library,
        "Laufend",
        DocumentFormat::Txt,
        "Gartenplanung aktiv",
    )
    .await;
    let archived = document(&library, "Alt", DocumentFormat::Txt, "Gartenplanung früher").await;
    let trashed = document(
        &library,
        "Weg",
        DocumentFormat::Txt,
        "Gartenplanung verworfen",
    )
    .await;
    documents.set_archived(archived.id, true).await.unwrap();
    documents.move_to_trash(trashed.id).await.unwrap();

    let hits = search(&library, "Gartenplanung").await;
    assert_eq!(ids(&hits), [active.id.to_string()]);

    let with_archive = SearchOptions {
        include_archived: true,
        ..options()
    };
    let hits = search_with(&library, "Gartenplanung", &with_archive).await;
    assert!(found(&hits, &active) && found(&hits, &archived) && !found(&hits, &trashed));
    let archived_hit = hits
        .iter()
        .find(|hit| hit.id == archived.id.to_string())
        .unwrap();
    assert!(archived_hit.archived);

    let with_trash = SearchOptions {
        include_trash: true,
        ..options()
    };
    let hits = search_with(&library, "Gartenplanung", &with_trash).await;
    assert!(found(&hits, &active) && !found(&hits, &archived) && found(&hits, &trashed));
    assert!(hits.iter().any(|hit| hit.deleted));
    library.close().await;
}

#[tokio::test]
async fn the_index_is_right_after_editing_deleting_and_restoring() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let documents = library.documents();
    let structure = library.structure();
    let note = document(
        &library,
        "Notiz",
        DocumentFormat::Md,
        "Erster Entwurf über Leuchttürme",
    )
    .await;
    assert_consistent(&library).await;

    // Editing: the old word goes, the new one comes.
    documents
        .save(note.id, "Zweiter Entwurf über Windmühlen".as_bytes())
        .await
        .unwrap();
    assert!(search(&library, "Leuchttürme").await.is_empty());
    assert_eq!(
        ids(&search(&library, "Windmuehlen").await),
        [note.id.to_string()]
    );

    // Renaming.
    documents.rename(note.id, "Küstenreise").await.unwrap();
    assert_eq!(
        ids(&search(&library, "kuestenreise").await),
        [note.id.to_string()]
    );
    assert!(search(&library, "Notiz").await.is_empty());

    // Tags: given, renamed, taken away.
    structure.tag_document(note.id, "Nordsee").await.unwrap();
    assert_eq!(
        ids(&search(&library, "Nordsee").await),
        [note.id.to_string()]
    );
    let tag = structure.tags().await.unwrap()[0].id.clone();
    structure
        .rename_tag(Uuid::try_parse(&tag).unwrap(), "Ostsee")
        .await
        .unwrap();
    assert!(search(&library, "Nordsee").await.is_empty());
    assert_eq!(
        ids(&search(&library, "Ostsee").await),
        [note.id.to_string()]
    );
    structure
        .untag_document(note.id, Uuid::try_parse(&tag).unwrap())
        .await
        .unwrap();
    assert!(search(&library, "Ostsee").await.is_empty());

    // Subnets: assigned, renamed; the subnet is found itself.
    let outcome = structure
        .create_subnet("Reisen", None, None, "Pläne für Urlaube")
        .await
        .unwrap();
    let subnet = Uuid::try_parse(outcome.subject_id.as_deref().unwrap()).unwrap();
    structure.assign(note.id, subnet).await.unwrap();
    let hits = search(&library, "Reisen").await;
    assert!(hits.iter().any(|hit| hit.kind == SearchKind::Subnet));
    assert!(found(&hits, &note));
    structure
        .update_subnet(subnet, Some("Fahrten"), None, None)
        .await
        .unwrap();
    assert!(search(&library, "Reisen").await.is_empty());
    let hits = search(&library, "Fahrten").await;
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!(
        search(&library, "Urlaube").await[0].kind,
        SearchKind::Subnet
    );
    assert_consistent(&library).await;

    // Deleting: in the trash only through the filter, back after restoring,
    // gone for good after purging.
    documents.move_to_trash(note.id).await.unwrap();
    assert!(search(&library, "Windmühlen").await.is_empty());
    documents.restore_from_trash(note.id).await.unwrap();
    assert_eq!(
        ids(&search(&library, "Windmühlen").await),
        [note.id.to_string()]
    );
    structure.set_deleted(note.id, true).await.unwrap();
    assert!(search(&library, "Windmühlen").await.is_empty());
    structure.undo().await.unwrap();
    assert_eq!(
        ids(&search(&library, "Windmühlen").await),
        [note.id.to_string()]
    );

    documents.move_to_trash(note.id).await.unwrap();
    documents.purge(note.id).await.unwrap();
    let everything = SearchOptions {
        include_archived: true,
        include_trash: true,
        ..options()
    };
    assert!(
        search_with(&library, "Windmühlen", &everything)
            .await
            .is_empty()
    );
    // The subnet stays; deleting it removes its entry.
    structure.delete_subnet(subnet, false).await.unwrap();
    assert!(search(&library, "Fahrten").await.is_empty());
    assert_consistent(&library).await;
    library.close().await;
}

#[tokio::test]
async fn media_are_found_by_name_description_tags_and_subnets() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let medium = import_svg(&library, "leuchtturm-skizze.svg").await;
    let id = medium.to_string();

    // The file name is metadata; the title starts as the name.
    let hits = search(&library, "leuchtturm").await;
    assert_eq!(ids(&hits), std::slice::from_ref(&id));
    assert_eq!(hits[0].kind, SearchKind::Image);

    library
        .media()
        .update_details(medium, Some("Westerhever"), Some("Rot und weiß gestreift"))
        .await
        .unwrap();
    assert_eq!(
        ids(&search(&library, "Westerhever").await),
        std::slice::from_ref(&id)
    );
    assert_eq!(
        ids(&search(&library, "weiss gestreift").await),
        std::slice::from_ref(&id)
    );
    // Still found by its file name.
    assert_eq!(
        ids(&search(&library, "skizze").await),
        std::slice::from_ref(&id)
    );

    let structure = library.structure();
    structure
        .tag_element(ElementKind::Media, medium, "Nordfriesland")
        .await
        .unwrap();
    assert_eq!(
        ids(&search(&library, "Nordfriesland").await),
        std::slice::from_ref(&id)
    );

    let only_documents = SearchOptions {
        kinds: vec![SearchKind::Document],
        ..options()
    };
    assert!(
        search_with(&library, "Westerhever", &only_documents)
            .await
            .is_empty()
    );
    let only_images = SearchOptions {
        kinds: vec![SearchKind::Image, SearchKind::Pdf],
        ..options()
    };
    assert_eq!(
        ids(&search_with(&library, "Westerhever", &only_images).await),
        [id]
    );
    assert_consistent(&library).await;
    library.close().await;
}

#[tokio::test]
async fn filters_by_tags_subnet_favourites_and_date() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let structure = library.structure();
    let a = document(
        &library,
        "Apfel",
        DocumentFormat::Txt,
        "Obstgarten Sorte eins",
    )
    .await;
    let b = document(
        &library,
        "Birne",
        DocumentFormat::Txt,
        "Obstgarten Sorte zwei",
    )
    .await;
    let c = document(
        &library,
        "Kirsche",
        DocumentFormat::Txt,
        "Obstgarten Sorte drei",
    )
    .await;
    structure.tag_document(a.id, "rot").await.unwrap();
    structure.tag_document(a.id, "süß").await.unwrap();
    structure.tag_document(b.id, "rot").await.unwrap();
    let tag = |name: &str| {
        let structure = structure.clone();
        let name = name.to_owned();
        async move {
            let tags = structure.tags().await.unwrap();
            Uuid::try_parse(&tags.iter().find(|tag| tag.name == name).unwrap().id).unwrap()
        }
    };
    let red = tag("rot").await;
    let sweet = tag("süß").await;

    let by_tags = |tags: Vec<Uuid>| SearchOptions { tags, ..options() };
    let mut hits = ids(&search_with(&library, "Obstgarten", &by_tags(vec![red])).await);
    hits.sort();
    let mut expected = vec![a.id.to_string(), b.id.to_string()];
    expected.sort();
    assert_eq!(hits, expected);
    // Every tag must be on the element.
    assert_eq!(
        ids(&search_with(&library, "Obstgarten", &by_tags(vec![red, sweet])).await),
        [a.id.to_string()]
    );

    // A subnet and its children.
    let top = structure
        .create_subnet("Garten", None, None, "")
        .await
        .unwrap();
    let top = Uuid::try_parse(top.subject_id.as_deref().unwrap()).unwrap();
    let child = structure
        .create_subnet("Beete", Some(top), None, "")
        .await
        .unwrap();
    let child = Uuid::try_parse(child.subject_id.as_deref().unwrap()).unwrap();
    structure.assign(b.id, child).await.unwrap();
    let in_top = SearchOptions {
        subnet: Some(top),
        ..options()
    };
    assert_eq!(
        ids(&search_with(&library, "Obstgarten", &in_top).await),
        [b.id.to_string()]
    );
    let in_child = SearchOptions {
        subnet: Some(child),
        ..options()
    };
    assert_eq!(
        ids(&search_with(&library, "Obstgarten", &in_child).await),
        [b.id.to_string()]
    );
    // The child subnet is found within its parent, by its name first, then
    // the document in it by the name of its subnet.
    let hits = search_with(&library, "Beete", &in_top).await;
    assert_eq!(ids(&hits), [child.to_string(), b.id.to_string()]);
    assert_eq!(hits[0].kind, SearchKind::Subnet);

    library.documents().set_favorite(c.id, true).await.unwrap();
    let favourites = SearchOptions {
        favorites_only: true,
        ..options()
    };
    assert_eq!(
        ids(&search_with(&library, "Obstgarten", &favourites).await),
        [c.id.to_string()]
    );

    let future = SearchOptions {
        updated_from: Some("2999-01-01T00:00:00.000Z".into()),
        ..options()
    };
    assert!(
        search_with(&library, "Obstgarten", &future)
            .await
            .is_empty()
    );
    let past = SearchOptions {
        updated_before: Some("2999-01-01T00:00:00.000Z".into()),
        updated_from: Some("2000-01-01T00:00:00.000Z".into()),
        ..options()
    };
    assert_eq!(search_with(&library, "Obstgarten", &past).await.len(), 3);

    let one = SearchOptions {
        limit: 1,
        ..options()
    };
    assert_eq!(search_with(&library, "Obstgarten", &one).await.len(), 1);
    library.close().await;
}

#[tokio::test]
async fn titles_rank_before_bodies() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let body = document(
        &library,
        "Notizen",
        DocumentFormat::Txt,
        "Etwas über Bienen und Honig",
    )
    .await;
    let title = document(
        &library,
        "Bienen",
        DocumentFormat::Txt,
        "Ein Volk im Garten",
    )
    .await;
    let hits = search(&library, "Bienen").await;
    assert_eq!(ids(&hits), [title.id.to_string(), body.id.to_string()]);
    library.close().await;
}

#[tokio::test]
async fn opening_fills_in_missing_bodies_and_the_index_can_be_rebuilt() {
    let dir = common::temp_dir("pw-search-");
    let path = dir.path().join("lib");
    let library = create(&path).await;
    let note = document(
        &library,
        "Logbuch",
        DocumentFormat::Pwrite,
        "Kompass und Sextant",
    )
    .await;
    import_svg(&library, "karte.svg").await;
    library
        .structure()
        .create_subnet("Seefahrt", None, None, "")
        .await
        .unwrap();

    // As right after migration 0009: no bodies, nothing indexed.
    sqlx::query("UPDATE search_words SET body = '' WHERE rowid IN (SELECT id FROM search_entries WHERE document_id IS NOT NULL)")
        .execute(library.database().writer())
        .await
        .unwrap();
    sqlx::query("UPDATE search_parts SET body = '' WHERE rowid IN (SELECT id FROM search_entries WHERE document_id IS NOT NULL)")
        .execute(library.database().writer())
        .await
        .unwrap();
    sqlx::query("UPDATE search_entries SET indexed_sha256 = NULL")
        .execute(library.database().writer())
        .await
        .unwrap();
    assert!(search(&library, "Sextant").await.is_empty());
    library.close().await;

    let library = open(&path).await;
    assert_eq!(
        ids(&search(&library, "Sextant").await),
        [note.id.to_string()]
    );
    assert_consistent(&library).await;

    // Rebuilt from scratch: the same hits.
    let before = search(&library, "Kompass").await;
    let report = library.search().rebuild().await.unwrap();
    assert_eq!(report.indexed, 1);
    assert_eq!(report.unreadable, 0);
    assert_eq!(search(&library, "Kompass").await, before);
    assert_eq!(search(&library, "karte").await.len(), 1);
    assert_eq!(search(&library, "Seefahrt").await.len(), 1);
    assert_consistent(&library).await;
    library.close().await;
}

#[tokio::test]
async fn links_and_media_ids_are_not_indexed() {
    let dir = common::temp_dir("pw-search-");
    let library = create(&dir.path().join("lib")).await;
    let target = document(&library, "Ziel", DocumentFormat::Txt, "").await;
    let source = document(
        &library,
        "Quelle",
        DocumentFormat::Md,
        &format!("Siehe [das Ziel](pwrite://{})", target.id),
    )
    .await;
    assert_eq!(
        ids(&search(&library, "Siehe").await),
        [source.id.to_string()]
    );
    let part_of_id: String = target.id.to_string().chars().take(8).collect();
    assert!(search(&library, &part_of_id).await.is_empty());
    library.close().await;
}
