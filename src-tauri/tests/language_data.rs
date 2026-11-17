//! Own dictionaries, rejections and language profiles in a real library
//! (section 14, ADR 0025): a document gets the union of the dictionaries
//! for all, of itself, of its subnets and of their parents.

mod common;

use project_write_lib::clock::Generators;
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::language::own::{DocumentLanguage, OwnError, WordCategory};
use project_write_lib::services::library::Library;
use uuid::Uuid;

struct Fixture {
    _dir: tempfile::TempDir,
    library: Library,
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-language-");
    let (library, _) = Library::create(&dir.path().join("lib"), Generators::system())
        .await
        .unwrap();
    Fixture { _dir: dir, library }
}

async fn subnet(library: &Library, name: &str, parent: Option<&str>) -> String {
    library
        .structure()
        .create_subnet(name, parent.map(|p| Uuid::parse_str(p).unwrap()), None, "")
        .await
        .unwrap()
        .subject_id
        .unwrap()
}

async fn document(library: &Library, title: &str) -> String {
    library
        .documents()
        .create(title, DocumentFormat::Md, b"Text")
        .await
        .unwrap()
        .id
        .to_string()
}

#[tokio::test]
async fn a_document_gets_the_union_of_its_dictionaries() {
    let f = fixture().await;
    let library = &f.library;
    let data = library.language();
    let wissen = subnet(library, "Wissen", None).await;
    let physik = subnet(library, "Physik", Some(&wissen)).await;
    let inside = document(library, "Teilchen").await;
    let outside = document(library, "Einkauf").await;
    library
        .structure()
        .assign(
            Uuid::parse_str(&inside).unwrap(),
            Uuid::parse_str(&physik).unwrap(),
        )
        .await
        .unwrap();

    let all = data.create("Eigene Wörter", true).await.unwrap();
    let fach = data.create("Fachbegriffe", false).await.unwrap();
    let solo = data.create("Nur hier", false).await.unwrap();
    data.add_word(&all.id, "Tauri", WordCategory::Name)
        .await
        .unwrap();
    data.add_word(&fach.id, "Quark-Gluon-Plasma", WordCategory::Term)
        .await
        .unwrap();
    data.add_word(&solo.id, "Einkaufszettelchen", WordCategory::Word)
        .await
        .unwrap();
    // The parent subnet's dictionary reaches the document in the child.
    data.assign(&fach.id, false, &[], std::slice::from_ref(&wissen))
        .await
        .unwrap();
    data.assign(&solo.id, false, std::slice::from_ref(&outside), &[])
        .await
        .unwrap();

    let words_inside = data.own_words(Some(&inside)).await.unwrap();
    assert!(words_inside.contains("Tauri"));
    assert!(words_inside.contains("Quark-Gluon-Plasma"));
    assert!(!words_inside.contains("Einkaufszettelchen"));
    let words_outside = data.own_words(Some(&outside)).await.unwrap();
    assert!(words_outside.contains("Tauri"));
    assert!(!words_outside.contains("Quark-Gluon-Plasma"));
    assert!(words_outside.contains("Einkaufszettelchen"));
    // An external file gets the dictionaries for all.
    let external = data.own_words(None).await.unwrap();
    assert_eq!(external.len(), 1);

    let listed = data.dictionaries().await.unwrap();
    let fach = listed.iter().find(|d| d.name == "Fachbegriffe").unwrap();
    assert_eq!(fach.subnets, [wissen.clone()]);
    assert_eq!(fach.words, 1);
    // A subnet deleted takes its targets with it.
    assert!(matches!(
        data.assign(
            &fach.id,
            false,
            &[],
            &["00000000-0000-4000-8000-000000000000".to_owned()]
        )
        .await,
        Err(OwnError::BadTarget)
    ));
}

#[tokio::test]
async fn words_are_added_changed_removed_imported_and_exported() {
    let f = fixture().await;
    let data = f.library.language();
    let own = data.create("Projekt", true).await.unwrap();
    assert!(matches!(
        data.create("projekt", false).await,
        Err(OwnError::NameTaken)
    ));
    assert!(
        data.add_word(&own.id, "Rust", WordCategory::Name)
            .await
            .unwrap()
    );
    assert!(
        !data
            .add_word(&own.id, "Rust", WordCategory::Term)
            .await
            .unwrap()
    );
    assert!(matches!(
        data.add_word(&own.id, "zwei Wörter", WordCategory::Word)
            .await,
        Err(OwnError::BadWord)
    ));
    data.update_word(&own.id, "Rust", "Rustacean", WordCategory::Word)
        .await
        .unwrap();
    assert!(matches!(
        data.remove_word(&own.id, "Rust").await,
        Err(OwnError::WordNotFound)
    ));
    let report = data
        .import_words(
            &own.id,
            "\u{feff}Tauri\nSQLx\nRustacean\nzwei Wörter\nTauri\n",
        )
        .await
        .unwrap();
    assert_eq!((report.added, report.known, report.skipped), (2, 2, 1));
    assert_eq!(
        data.export_words(&own.id).await.unwrap(),
        "Rustacean\nSQLx\nTauri\n"
    );
    data.remove_word(&own.id, "SQLx").await.unwrap();
    let renamed = data.rename(&own.id, "Projektwörter").await.unwrap();
    assert_eq!(renamed.words, 2);
    data.delete(&own.id).await.unwrap();
    assert!(matches!(data.words(&own.id).await, Err(OwnError::NotFound)));
}

#[tokio::test]
async fn add_to_dictionary_makes_the_first_dictionary_for_all() {
    let f = fixture().await;
    let data = f.library.language();
    let first = data
        .add_to_default("Flohmarktstand", "Eigene Wörter")
        .await
        .unwrap();
    assert!(first.for_all);
    let again = data.add_to_default("Tauri", "Eigene Wörter").await.unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(again.words, 2);
    assert!(
        data.own_words(None)
            .await
            .unwrap()
            .contains("Flohmarktstand")
    );
}

#[tokio::test]
async fn rejections_and_profiles_are_kept() {
    let f = fixture().await;
    let data = f.library.language();
    let id = document(&f.library, "Brief").await;
    data.reject("Mitter", "Mutter").await.unwrap();
    data.reject("Mitter", "Mutter").await.unwrap();
    assert!(data.is_rejected("Mitter", "Mutter").await.unwrap());
    assert_eq!(data.rejected_for("Mitter").await.unwrap().len(), 1);
    assert_eq!(data.rejections().await.unwrap().len(), 1);
    data.forget_rejection("Mitter", "Mutter").await.unwrap();
    assert!(!data.is_rejected("Mitter", "Mutter").await.unwrap());

    assert_eq!(
        data.profile(&id).await.unwrap(),
        DocumentLanguage::default()
    );
    let profile = DocumentLanguage {
        languages: vec!["de-CH".to_owned(), "en-GB".to_owned(), "de-CH".to_owned()],
        autocorrect: false,
    };
    let stored = data.set_profile(&id, &profile).await.unwrap();
    assert_eq!(stored.languages, ["de-CH", "en-GB"]);
    assert_eq!(data.profile(&id).await.unwrap(), stored);
    let bad = DocumentLanguage {
        languages: vec!["../x".to_owned()],
        autocorrect: true,
    };
    assert!(matches!(
        data.set_profile(&id, &bad).await,
        Err(OwnError::BadLanguage)
    ));
}
