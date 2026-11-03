//! Document repository (Phase 2): files with UUID names, metadata in the
//! database, atomic saves, no writes through links.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use project_write_lib::clock::Generators;
use project_write_lib::security::paths::PathError;
use project_write_lib::services::documents::{
    DocumentError, DocumentFormat, MAX_DOCUMENT_BYTES, SaveOutcome, sha256_hex,
};
use project_write_lib::services::library::{Library, OpenOptions};
use uuid::Uuid;

struct Fixture {
    dir: tempfile::TempDir,
    library: Library,
}

impl Fixture {
    fn path(&self) -> PathBuf {
        self.library.path().to_path_buf()
    }

    fn file(&self, format: DocumentFormat, id: Uuid) -> PathBuf {
        self.path().join(format.relative_path(id))
    }
}

async fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-docs-");
    let (library, _) = Library::create(&dir.path().join("lib"), Generators::system())
        .await
        .unwrap();
    Fixture { dir, library }
}

#[tokio::test]
async fn create_stores_a_uuid_named_file_and_a_row() {
    let f = fixture().await;
    let docs = f.library.documents();
    let content = "Überschrift\n\nErster Absatz mit Umlauten: äöüß.\n".as_bytes();
    let meta = docs
        .create("  Mein erstes Dokument ", DocumentFormat::Txt, content)
        .await
        .unwrap();
    assert_eq!(meta.title, "Mein erstes Dokument");
    assert_eq!(meta.format, DocumentFormat::Txt);
    assert_eq!(meta.size_bytes, content.len() as u64);
    assert_eq!(meta.content_sha256, sha256_hex(content));
    assert_eq!(meta.created_at, meta.updated_at);
    assert_eq!(meta.deleted_at, None);
    let file = f.file(DocumentFormat::Txt, meta.id);
    assert_eq!(fs::read(&file).unwrap(), content);
    assert_eq!(
        file.file_name().unwrap().to_str().unwrap(),
        format!("{}.txt", meta.id)
    );

    for format in [DocumentFormat::Md, DocumentFormat::Pwrite] {
        let other = docs.create("Weiteres", format, b"{}").await.unwrap();
        assert!(f.file(format, other.id).is_file());
    }
    assert_eq!(docs.count().await.unwrap(), 3);
    assert_eq!(docs.list().await.unwrap().len(), 3);
    let (read_meta, read_content) = docs.read(meta.id).await.unwrap();
    assert_eq!(read_meta, meta);
    assert_eq!(read_content, content);
    f.library.close().await;
}

#[tokio::test]
async fn save_replaces_content_and_skips_unchanged_content() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Notiz", DocumentFormat::Md, b"# Alt")
        .await
        .unwrap();
    let file = f.file(DocumentFormat::Md, meta.id);

    let saved = docs.save(meta.id, b"# Neu\n\nMehr Text").await.unwrap();
    let SaveOutcome::Saved(saved) = saved else {
        panic!("expected Saved, got {saved:?}");
    };
    assert_eq!(fs::read(&file).unwrap(), b"# Neu\n\nMehr Text");
    assert_eq!(saved.content_sha256, sha256_hex(b"# Neu\n\nMehr Text"));
    assert_eq!(saved.size_bytes, 16);
    assert!(saved.updated_at >= meta.updated_at);

    // Same content again: nothing is written.
    let modified = fs::metadata(&file).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(matches!(
        docs.save(meta.id, b"# Neu\n\nMehr Text").await.unwrap(),
        SaveOutcome::Unchanged(_)
    ));
    assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);

    // Empty content is valid.
    docs.save(meta.id, b"").await.unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"");
    let leftovers: Vec<_> = fs::read_dir(file.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    f.library.close().await;
}

#[tokio::test]
async fn rename_changes_only_the_title() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Alt", DocumentFormat::Txt, b"Text")
        .await
        .unwrap();
    let file = f.file(DocumentFormat::Txt, meta.id);
    let renamed = docs.rename(meta.id, "Neuer Titel").await.unwrap();
    assert_eq!(renamed.title, "Neuer Titel");
    assert_eq!(renamed.content_sha256, meta.content_sha256);
    assert_eq!(fs::read(&file).unwrap(), b"Text");
    assert!(matches!(
        docs.rename(meta.id, " \t ").await,
        Err(DocumentError::InvalidTitle(_))
    ));
    assert!(matches!(
        docs.rename(Uuid::new_v4(), "x").await,
        Err(DocumentError::NotFound)
    ));
    f.library.close().await;
}

#[tokio::test]
async fn trash_keeps_row_and_file() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Weg", DocumentFormat::Txt, b"Text")
        .await
        .unwrap();
    let trashed = docs.move_to_trash(meta.id).await.unwrap();
    assert!(trashed.deleted_at.is_some());
    assert_eq!(docs.count().await.unwrap(), 0);
    assert!(docs.list().await.unwrap().is_empty());
    assert!(f.file(DocumentFormat::Txt, meta.id).is_file());
    assert!(matches!(
        docs.save(meta.id, b"neu").await,
        Err(DocumentError::InTrash)
    ));
    assert!(matches!(
        docs.move_to_trash(meta.id).await,
        Err(DocumentError::InTrash)
    ));
    // Still readable (restoring from the trash comes in Phase 6).
    assert_eq!(docs.read(meta.id).await.unwrap().1, b"Text");
    f.library.close().await;
}

#[tokio::test]
async fn invalid_input_is_rejected_before_anything_is_written() {
    let f = fixture().await;
    let docs = f.library.documents();
    for title in ["", "   ", "a\nb", &"x".repeat(301)] {
        assert!(matches!(
            docs.create(title, DocumentFormat::Txt, b"x").await,
            Err(DocumentError::InvalidTitle(_))
        ));
    }
    let too_large = vec![b'x'; usize::try_from(MAX_DOCUMENT_BYTES).unwrap() + 1];
    assert!(matches!(
        docs.create("Groß", DocumentFormat::Txt, &too_large).await,
        Err(DocumentError::TooLarge)
    ));
    assert_eq!(
        fs::read_dir(f.path().join("documents/text"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(docs.count().await.unwrap(), 0);
    f.library.close().await;
}

#[tokio::test]
async fn a_changed_file_wins_and_corrects_the_row() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Extern", DocumentFormat::Txt, b"alt")
        .await
        .unwrap();
    // As after a crash between renaming the file and updating the row.
    fs::write(f.file(DocumentFormat::Txt, meta.id), b"neuer Inhalt").unwrap();
    let (corrected, content) = docs.read(meta.id).await.unwrap();
    assert_eq!(content, b"neuer Inhalt");
    assert_eq!(corrected.content_sha256, sha256_hex(b"neuer Inhalt"));
    assert_eq!(corrected.size_bytes, 12);
    assert_eq!(docs.get(meta.id).await.unwrap().unwrap(), corrected);
    f.library.close().await;
}

#[tokio::test]
async fn documents_are_never_read_or_written_through_links() {
    let f = fixture().await;
    let docs = f.library.documents();
    let meta = docs
        .create("Ziel", DocumentFormat::Txt, b"eigen")
        .await
        .unwrap();
    let file = f.file(DocumentFormat::Txt, meta.id);
    let outside = f.dir.path().join("outside.txt");
    fs::write(&outside, b"fremde Datei").unwrap();
    fs::remove_file(&file).unwrap();
    common::symlink_file(&outside, &file)
        .expect("create file symlink (Windows: needs admin rights or developer mode)");

    assert!(matches!(
        docs.read(meta.id).await,
        Err(DocumentError::Path(PathError::SymbolicLink { .. }))
    ));
    assert!(matches!(
        docs.save(meta.id, b"neu").await,
        Err(DocumentError::Path(PathError::SymbolicLink { .. }))
    ));
    assert_eq!(fs::read(&outside).unwrap(), b"fremde Datei");
    f.library.close().await;
}

#[tokio::test]
async fn documents_survive_closing_and_reopening() {
    let f = fixture().await;
    let path = f.path();
    let docs = f.library.documents();
    let a = docs
        .create("A", DocumentFormat::Txt, b"eins")
        .await
        .unwrap();
    let b = docs.create("B", DocumentFormat::Md, b"zwei").await.unwrap();
    docs.save(a.id, "eins, geändert".as_bytes()).await.unwrap();
    f.library.close().await;

    let (library, report) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert!(report.documents.orphaned_files.is_empty());
    let docs = library.documents();
    assert_eq!(
        docs.read(a.id).await.unwrap().1,
        "eins, geändert".as_bytes()
    );
    assert_eq!(docs.read(b.id).await.unwrap().1, b"zwei");
    library.close().await;
    drop(f.dir);
    assert!(!Path::new(&path).exists());
}
