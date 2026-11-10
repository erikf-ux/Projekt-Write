//! Media (Phase 8, sections 9, 17 and 20; ADR 0019). Acceptance of the
//! phase:
//!
//! * "Originaldateien bleiben bytegleich, belegt durch Prüfsummen vor und
//!   nach dem Import."
//! * "Ein abgebrochener Import hinterlässt weder Dateien noch
//!   Datenbankeinträge." – cancelled after preparing, a failure injected
//!   while committing, and hard kills of a helper process that imports
//!   without pause.
//!
//! The SVG and PDF checks of the acceptance run in the real app
//! (`tests/e2e/specs/media.e2e.ts`); the headers of the media handler are
//! tested in `tests/ipc.rs`.

mod common;

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use common::SplitMix64;
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use project_write_lib::clock::{Generators, IdSource, SystemClock};
use project_write_lib::database::diagnostics;
use project_write_lib::services::documents::{DocumentFormat, sha256_hex};
use project_write_lib::services::library::{Library, LibraryError, OpenOptions};
use project_write_lib::services::media::{
    Decision, Duplicate, ImportSource, MediaError, MediaKind, SkipReason, SubnetPlacement,
    import::walk_folder, scan_media,
};
use project_write_lib::services::settings::library::MediaMaxMegapixels;
use project_write_lib::services::structure::ElementKind;
use uuid::Uuid;

// --- Sample files ------------------------------------------------------------------

fn encoded(image: &image::DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), format)
        .unwrap();
    bytes
}

fn png(width: u32, height: u32, shade: u8) -> Vec<u8> {
    encoded(
        &RgbImage::from_pixel(width, height, Rgb([shade, 80, 160])).into(),
        ImageFormat::Png,
    )
}

fn jpeg(shade: u8) -> Vec<u8> {
    encoded(
        &RgbImage::from_pixel(64, 48, Rgb([shade, 120, 40])).into(),
        ImageFormat::Jpeg,
    )
}

fn gif() -> Vec<u8> {
    encoded(
        &RgbaImage::from_pixel(32, 32, Rgba([10, 200, 10, 255])).into(),
        ImageFormat::Gif,
    )
}

fn webp() -> Vec<u8> {
    encoded(
        &RgbaImage::from_pixel(40, 30, Rgba([200, 10, 10, 128])).into(),
        ImageFormat::WebP,
    )
}

const SVG: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#3a7"/></svg>
"##;

/// A valid one-page PDF with a line of text.
fn pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 18 Tf 72 720 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

fn docx(main_type: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(
                format!(
                    r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="{main_type}"/></Types>"#
                )
                .as_bytes(),
            )
            .unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(b"<w:document/>").unwrap();
        for (name, content) in extra {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

const DOCX_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Ids handed out in a given order first, random ones afterwards.
#[derive(Default)]
struct QueuedIds(Mutex<VecDeque<Uuid>>);

impl IdSource for QueuedIds {
    fn new_id(&self) -> Uuid {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(Uuid::new_v4)
    }
}

/// A file with its bytes, checksum and modification time.
struct Original {
    path: PathBuf,
    bytes: Vec<u8>,
    sha256: String,
    modified: std::time::SystemTime,
}

fn write_original(dir: &Path, name: &str, bytes: &[u8]) -> Original {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    Original {
        modified: fs::metadata(&path).unwrap().modified().unwrap(),
        path,
        bytes: bytes.to_vec(),
        sha256: sha256_hex(bytes),
    }
}

fn assert_unchanged(original: &Original) {
    let now = fs::read(&original.path).unwrap();
    assert_eq!(now, original.bytes, "{} changed", original.path.display());
    assert_eq!(sha256_hex(&now), original.sha256);
    assert_eq!(
        fs::metadata(&original.path).unwrap().modified().unwrap(),
        original.modified,
        "{} was written",
        original.path.display()
    );
}

fn source(original: &Original) -> ImportSource {
    ImportSource {
        path: original.path.clone(),
        name: original
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        folders: Vec::new(),
    }
}

fn no_progress() -> Arc<dyn Fn(usize, usize) + Send + Sync> {
    Arc::new(|_, _| {})
}

async fn media_rows(library: &Library) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM media")
        .fetch_one(library.database().reader())
        .await
        .unwrap()
}

/// Files below `media/` (all kinds).
fn media_files(path: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for kind in ["images", "pdf", "attachments"] {
        if let Ok(entries) = fs::read_dir(path.join("media").join(kind)) {
            for entry in entries.flatten() {
                found.push(format!("{kind}/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    found.sort();
    found
}

/// Entries of `tmp/` that belong to imports.
fn import_leftovers(path: &Path) -> Vec<String> {
    fs::read_dir(path.join("tmp"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("import-"))
                .collect()
        })
        .unwrap_or_default()
}

// --- Byte-identical originals --------------------------------------------------------

#[tokio::test]
async fn originals_stay_byte_identical_and_the_copies_equal_them() {
    let dir = common::temp_dir("pw-media-bytes-");
    let originals_dir = dir.path().join("originale");
    let originals = [
        write_original(&originals_dir, "Foto.png", &png(120, 80, 1)),
        write_original(&originals_dir, "Foto.JPG", &jpeg(2)),
        write_original(&originals_dir, "Animation.gif", &gif()),
        write_original(&originals_dir, "Transparent.webp", &webp()),
        write_original(&originals_dir, "Logo.svg", SVG.as_bytes()),
        write_original(&originals_dir, "Quelle.pdf", &pdf("Seite eins")),
        write_original(&originals_dir, "Bericht.docx", &docx(DOCX_MAIN, &[])),
    ];
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let media = library.media();

    let preview = media
        .prepare(
            Uuid::new_v4(),
            originals.iter().map(source).collect(),
            Vec::new(),
            None,
            Some(originals_dir.display().to_string()),
            no_progress(),
        )
        .await
        .unwrap();
    assert_eq!(preview.items.len(), originals.len());
    assert!(preview.skipped.is_empty(), "{:?}", preview.skipped);
    let batch = Uuid::try_parse(&preview.batch_id).unwrap();
    let outcome = media
        .commit_import(
            batch,
            &BTreeMap::new(),
            Decision::Import,
            &SubnetPlacement::default(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.items.len(), originals.len());

    for (original, item) in originals.iter().zip(&outcome.items) {
        assert_unchanged(original);
        let id = Uuid::try_parse(&item.id).unwrap();
        let summary = media.get(id).await.unwrap();
        let row_sha: String = sqlx::query_scalar("SELECT sha256 FROM media WHERE id = ?1")
            .bind(&item.id)
            .fetch_one(library.database().reader())
            .await
            .unwrap();
        assert_eq!(row_sha, original.sha256);
        assert_eq!(summary.size_bytes, original.bytes.len() as u64);
        assert_eq!(
            summary.import_folder.as_deref(),
            Some(originals_dir.display().to_string().as_str())
        );
        // The copy in the library equals the original byte for byte.
        let extension = original
            .path
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        let folder = match summary.kind {
            MediaKind::Image => "images",
            MediaKind::Pdf => "pdf",
            MediaKind::Attachment => "attachments",
        };
        let copy = fs::read(path.join(format!("media/{folder}/{}.{extension}", item.id))).unwrap();
        assert_eq!(copy, original.bytes);
    }
    let kinds: Vec<MediaKind> = media.list().await.unwrap().iter().map(|m| m.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == MediaKind::Image).count(), 5);
    assert!(kinds.contains(&MediaKind::Pdf) && kinds.contains(&MediaKind::Attachment));
    // Width and height come from the header of raster images.
    let photo = media
        .get(Uuid::try_parse(&outcome.items[0].id).unwrap())
        .await
        .unwrap();
    assert_eq!((photo.width, photo.height), (Some(120), Some(80)));
    assert_eq!(photo.title, "Foto");
    assert!(import_leftovers(&path).is_empty());
    assert_eq!(
        scan_media(library.root(), library.database())
            .await
            .unwrap(),
        Default::default()
    );
    library.close().await;

    // The library opens without complaint and the originals are untouched.
    let (library, _) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert!(
        diagnostics::integrity_check(library.database().writer())
            .await
            .unwrap()
    );
    library.close().await;
    for original in &originals {
        assert_unchanged(original);
    }
}

// --- Type check ------------------------------------------------------------------------

#[tokio::test]
async fn files_off_the_positive_list_are_skipped_with_a_reason() {
    let dir = common::temp_dir("pw-media-types-");
    let folder = dir.path().join("Mappe");
    let good = write_original(&folder, "gut.png", &png(10, 10, 3));
    write_original(&folder, "Programm.png", b"MZ\x90\0\x03\0\0\0 not an image");
    write_original(&folder, "falsch.jpg", &png(10, 10, 4));
    write_original(&folder, "Makro.docm", &docx(DOCX_MAIN, &[]));
    write_original(
        &folder,
        "Makro.docx",
        &docx(DOCX_MAIN, &[("word/vbaProject.bin", "Attribute VB_Name")]),
    );
    write_original(
        &folder,
        "Bombe.svg",
        b"<?xml version=\"1.0\"?><!DOCTYPE svg [<!ENTITY a \"aaaaaaaa\">]><svg>&a;</svg>",
    );
    write_original(&folder, "Notiz.md", b"# Notiz");
    write_original(&folder, "Tabelle.csv", b"a;b");
    write_original(&folder, ".versteckt.png", &png(10, 10, 5));
    #[cfg(unix)]
    std::os::unix::fs::symlink(&good.path, folder.join("Verweis.png")).unwrap();

    let (sources, skipped) = walk_folder(&folder).unwrap();
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let preview = library
        .media()
        .prepare(
            Uuid::new_v4(),
            sources,
            skipped,
            Some("Mappe".to_owned()),
            None,
            no_progress(),
        )
        .await
        .unwrap();
    let names: Vec<&str> = preview.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["gut.png"]);
    let reason = |name: &str| {
        preview
            .skipped
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.reason)
    };
    assert_eq!(reason("Programm.png"), Some(SkipReason::Type));
    assert_eq!(reason("falsch.jpg"), Some(SkipReason::Extension));
    assert_eq!(reason("Makro.docm"), Some(SkipReason::Macro));
    assert_eq!(reason("Makro.docx"), Some(SkipReason::Macro));
    assert_eq!(reason("Bombe.svg"), Some(SkipReason::Content));
    assert_eq!(reason("Notiz.md"), Some(SkipReason::Document));
    assert_eq!(reason("Tabelle.csv"), Some(SkipReason::Type));
    assert_eq!(reason(".versteckt.png"), None, "hidden files are left out");
    #[cfg(unix)]
    assert_eq!(reason("Verweis.png"), Some(SkipReason::Link));
    // The copies of refused files are gone already.
    let batch_dir = path
        .join("tmp")
        .join(format!("import-{}", preview.batch_id));
    assert_eq!(fs::read_dir(&batch_dir).unwrap().count(), 1);
    library
        .media()
        .cancel_import(Uuid::try_parse(&preview.batch_id).unwrap());
    library.close().await;
}

#[tokio::test]
async fn images_above_the_megapixel_limit_are_refused() {
    let dir = common::temp_dir("pw-media-pixels-");
    let big = write_original(&dir.path().join("in"), "gross.png", &png(1500, 1000, 6));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    library
        .settings()
        .set::<MediaMaxMegapixels>(&1)
        .await
        .unwrap();
    let preview = library
        .media()
        .prepare(
            Uuid::new_v4(),
            vec![source(&big)],
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    assert!(preview.items.is_empty());
    assert_eq!(preview.skipped[0].reason, SkipReason::Megapixels);
    library
        .settings()
        .set::<MediaMaxMegapixels>(&2)
        .await
        .unwrap();
    let preview = library
        .media()
        .prepare(
            Uuid::new_v4(),
            vec![source(&big)],
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    assert_eq!(preview.items.len(), 1);
    library.close().await;
}

// --- Duplicates ------------------------------------------------------------------------

#[tokio::test]
async fn duplicates_offer_the_existing_medium() {
    let dir = common::temp_dir("pw-media-dup-");
    let a = write_original(&dir.path().join("in"), "a.png", &png(20, 20, 7));
    let b = write_original(&dir.path().join("in"), "b.png", &a.bytes);
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let media = library.media();

    // Within one batch, the second is a duplicate of the first.
    let preview = media
        .prepare(
            Uuid::new_v4(),
            vec![source(&a), source(&b)],
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    assert_eq!(preview.items[0].duplicate, None);
    assert_eq!(
        preview.items[1].duplicate,
        Some(Duplicate::InBatch { index: 0 })
    );
    let outcome = media
        .commit_import(
            Uuid::try_parse(&preview.batch_id).unwrap(),
            &BTreeMap::new(),
            Decision::UseExisting,
            &SubnetPlacement::default(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.items[1].id, outcome.items[0].id);
    assert!(outcome.items[0].created && !outcome.items[1].created);
    assert_eq!(media_rows(&library).await, 1);

    // Later, the medium of the library is offered; "trotzdem importieren"
    // makes a second one.
    let preview = media
        .prepare(
            Uuid::new_v4(),
            vec![source(&b)],
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    assert_eq!(
        preview.items[0].duplicate,
        Some(Duplicate::Existing {
            id: outcome.items[0].id.clone(),
            title: "a".to_owned()
        })
    );
    let again = media
        .commit_import(
            Uuid::try_parse(&preview.batch_id).unwrap(),
            &BTreeMap::from([(0, Decision::Import)]),
            Decision::UseExisting,
            &SubnetPlacement::default(),
        )
        .await
        .unwrap();
    assert!(again.items[0].created);
    assert_eq!(media_rows(&library).await, 2);
    library.close().await;
}

// --- Aborted imports leave nothing -------------------------------------------------

#[tokio::test]
async fn a_cancelled_import_leaves_neither_files_nor_rows() {
    let dir = common::temp_dir("pw-media-cancel-");
    let originals: Vec<Original> = (0..5)
        .map(|i| write_original(&dir.path().join("in"), &format!("{i}.png"), &png(30, 30, i)))
        .collect();
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let preview = library
        .media()
        .prepare(
            Uuid::new_v4(),
            originals.iter().map(source).collect(),
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    let batch = Uuid::try_parse(&preview.batch_id).unwrap();
    assert_eq!(import_leftovers(&path).len(), 1, "the copies wait in tmp/");
    library.media().cancel_import(batch);
    assert!(import_leftovers(&path).is_empty());
    assert!(media_files(&path).is_empty());
    assert_eq!(media_rows(&library).await, 0);
    assert!(matches!(
        library
            .media()
            .commit_import(
                batch,
                &BTreeMap::new(),
                Decision::Import,
                &SubnetPlacement::default()
            )
            .await,
        Err(MediaError::ImportNotFound)
    ));
    library.close().await;
    for original in &originals {
        assert_unchanged(original);
    }
}

#[tokio::test]
async fn a_failure_while_committing_moves_every_file_back() {
    let dir = common::temp_dir("pw-media-fail-");
    let originals: Vec<Original> = (0..3)
        .map(|i| {
            write_original(
                &dir.path().join("in"),
                &format!("{i}.png"),
                &png(30, 30, 40 + i),
            )
        })
        .collect();
    let path = dir.path().join("lib");
    let ids = Arc::new(QueuedIds::default());
    let generators = Generators::new(Arc::new(SystemClock), ids.clone());
    let (library, _) = Library::create(&path, generators).await.unwrap();
    let preview = library
        .media()
        .prepare(
            Uuid::new_v4(),
            originals.iter().map(source).collect(),
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
    // The third file gets an id a row already has: its insert fails after
    // all three files were moved into media/.
    let planned: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
    ids.0.lock().unwrap().extend(planned.iter().copied());
    sqlx::query(
        "INSERT INTO media (id, kind, mime_type, extension, title, original_name, size_bytes, sha256, created_at, updated_at) \
         VALUES (?1, 'pdf', 'application/pdf', 'pdf', 'Besetzt', 'x.pdf', 0, \
                 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', \
                 '2026-11-10T10:00:00.000Z', '2026-11-10T10:00:00.000Z')",
    )
    .bind(planned[2].hyphenated().to_string())
    .execute(library.database().writer())
    .await
    .unwrap();
    let result = library
        .media()
        .commit_import(
            Uuid::try_parse(&preview.batch_id).unwrap(),
            &BTreeMap::new(),
            Decision::Import,
            &SubnetPlacement::default(),
        )
        .await;
    assert!(result.is_err(), "{result:?}");
    assert!(media_files(&path).is_empty(), "{:?}", media_files(&path));
    assert_eq!(media_rows(&library).await, 1, "only the row that was there");
    assert!(import_leftovers(&path).is_empty());
    library.close().await;
    for original in &originals {
        assert_unchanged(original);
    }
}

#[tokio::test]
async fn unfinished_imports_are_cleaned_up_when_the_library_opens() {
    let dir = common::temp_dir("pw-media-journal-");
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    library.close().await;
    // The state a crash between moving the files and committing leaves.
    let batch = Uuid::new_v4();
    let unfinished = Uuid::new_v4().hyphenated().to_string();
    let stray = Uuid::new_v4().hyphenated().to_string();
    fs::create_dir_all(path.join("media/images")).unwrap();
    fs::write(
        path.join(format!("media/images/{unfinished}.png")),
        png(5, 5, 9),
    )
    .unwrap();
    fs::write(
        path.join(format!("media/images/{stray}.png")),
        png(5, 5, 10),
    )
    .unwrap();
    fs::create_dir_all(path.join("tmp")).unwrap();
    fs::write(
        path.join(format!("tmp/import-{batch}.json")),
        serde_json::json!({
            "batch": batch.hyphenated().to_string(),
            "files": [
                { "id": unfinished, "relative": format!("media/images/{unfinished}.png") },
                // Never followed outside the media folders.
                { "id": unfinished, "relative": "library.json" }
            ]
        })
        .to_string(),
    )
    .unwrap();
    let (library, _) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    assert!(!path.join(format!("media/images/{unfinished}.png")).exists());
    assert!(path.join("library.json").exists());
    // A file no journal names is reported, never deleted (section 9).
    assert!(path.join(format!("media/images/{stray}.png")).exists());
    let scan = scan_media(library.root(), library.database())
        .await
        .unwrap();
    assert_eq!(scan.orphaned_files, [format!("media/images/{stray}.png")]);
    assert!(import_leftovers(&path).is_empty());
    library.close().await;
}

const LIBRARY_ENV: &str = "PW_TEST_MEDIA_LIBRARY";
const SOURCE_ENV: &str = "PW_TEST_MEDIA_SOURCE";
const KILLS: usize = 40;
const FILES_PER_BATCH: usize = 6;

async fn open_eventually(path: &Path) -> Library {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match Library::open(path, OpenOptions::default(), Generators::system()).await {
            Ok((library, _)) => return library,
            Err(LibraryError::Locked) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(error) => panic!("open failed: {error}"),
        }
    }
}

/// Helper process: imports the source folder again and again until killed.
#[test]
#[ignore = "helper process of hard_kills_while_importing_leave_no_partial_import"]
fn media_import_process() {
    let (Some(path), Some(source)) = (std::env::var_os(LIBRARY_ENV), std::env::var_os(SOURCE_ENV))
    else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let library = open_eventually(Path::new(&path)).await;
        let media = library.media();
        println!("\nPW-IMPORT-READY");
        let mut round = 0u64;
        loop {
            let (sources, skipped) = walk_folder(Path::new(&source)).unwrap();
            let preview = media
                .prepare(Uuid::new_v4(), sources, skipped, None, None, no_progress())
                .await
                .unwrap();
            println!("PW-IMPORT-COMMIT {round}");
            media
                .commit_import(
                    Uuid::try_parse(&preview.batch_id).unwrap(),
                    &BTreeMap::new(),
                    Decision::Import,
                    &SubnetPlacement::default(),
                )
                .await
                .unwrap();
            println!("PW-IMPORT-DONE {round}");
            round += 1;
        }
    });
}

/// Starts the helper; the second value yields the last line it printed.
fn spawn_importer(
    path: &Path,
    source: &Path,
) -> (std::process::Child, std::thread::JoinHandle<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "media_import_process",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LIBRARY_ENV, path)
        .env(SOURCE_ENV, source)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn helper process");
    let stdout = child.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut last = String::new();
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.contains("PW-IMPORT-READY") {
                let _ = ready_tx.send(());
            }
            if line.starts_with("PW-IMPORT-") {
                last = line;
            }
        }
        last
    });
    ready_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("helper process became ready");
    (child, reader)
}

#[test]
fn hard_kills_while_importing_leave_no_partial_import() {
    let dir = common::temp_dir("pw-media-kill-");
    let source = dir.path().join("quelle");
    let mut rng = SplitMix64(0x5EED_2026_1110);
    let originals: Vec<Original> = (0..FILES_PER_BATCH)
        .map(|i| {
            // 300 × 300 to 900 × 900 pixels: a few hundred kilobytes each.
            let side = rng.between(300..900) as u32;
            write_original(&source, &format!("{i}.png"), &png(side, side, i as u8))
        })
        .collect();
    let path = dir.path().join("lib");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
        library.close().await;
    });
    let (mut while_committing, mut while_preparing) = (0, 0);
    for kill in 0..KILLS {
        let (mut child, reader) = spawn_importer(&path, &source);
        std::thread::sleep(Duration::from_millis(rng.between(5..400)));
        child.kill().unwrap();
        child.wait().unwrap();
        let last = reader.join().unwrap();
        if last.starts_with("PW-IMPORT-COMMIT") {
            while_committing += 1;
        } else {
            while_preparing += 1;
        }
        runtime.block_on(async {
            let library = open_eventually(&path).await;
            assert!(
                diagnostics::integrity_check(library.database().writer())
                    .await
                    .unwrap(),
                "kill {kill}: integrity"
            );
            // Whole batches only: every file has its row, every row its file.
            let rows = media_rows(&library).await;
            assert_eq!(rows % FILES_PER_BATCH as i64, 0, "kill {kill}: {rows} rows");
            let scan = scan_media(library.root(), library.database())
                .await
                .unwrap();
            assert!(
                scan.orphaned_files.is_empty(),
                "kill {kill}: {:?}",
                scan.orphaned_files
            );
            assert!(
                scan.missing_files.is_empty(),
                "kill {kill}: {:?}",
                scan.missing_files
            );
            assert!(import_leftovers(&path).is_empty(), "kill {kill}: tmp");
            // Every copy equals its checksum.
            for medium in library.media().list().await.unwrap() {
                let data = fs::read(path.join(format!("media/images/{}.png", medium.id))).unwrap();
                assert_eq!(data.len() as u64, medium.size_bytes);
            }
            library.close().await;
        });
    }
    eprintln!(
        "{KILLS} kills: {while_committing} while committing, {while_preparing} while preparing"
    );
    assert!(while_committing > 0, "no kill landed in a commit");
    for original in &originals {
        assert_unchanged(original);
    }
}

// --- Folders and subnets ------------------------------------------------------------

#[tokio::test]
async fn folders_become_subnets_on_request() {
    let dir = common::temp_dir("pw-media-folders-");
    let folder = dir.path().join("Reise");
    write_original(&folder, "Ankunft.png", &png(10, 10, 11));
    write_original(&folder.join("Tag 1"), "Strand.jpg", &jpeg(12));
    write_original(
        &folder.join("Tag 1").join("Abend"),
        "Plan.pdf",
        &pdf("Plan"),
    );
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let media = library.media();
    let import = |media: project_write_lib::services::media::MediaLibrary| {
        let folder = folder.clone();
        async move {
            let (sources, skipped) = walk_folder(&folder).unwrap();
            let preview = media
                .prepare(
                    Uuid::new_v4(),
                    sources,
                    skipped,
                    Some("Reise".to_owned()),
                    Some(folder.display().to_string()),
                    no_progress(),
                )
                .await
                .unwrap();
            media
                .commit_import(
                    Uuid::try_parse(&preview.batch_id).unwrap(),
                    &BTreeMap::new(),
                    Decision::UseExisting,
                    &SubnetPlacement {
                        parent: None,
                        from_folders: true,
                    },
                )
                .await
                .unwrap()
        }
    };
    let outcome = import(media.clone()).await;
    let subnets = library.structure().subnets().await.unwrap();
    let name_of = |id: &str| subnets.iter().find(|s| s.id == id).unwrap().name.clone();
    let mut names: Vec<String> = subnets.iter().map(|s| s.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Abend", "Reise", "Tag 1"]);
    let reise = outcome.subnet_id.clone().unwrap();
    assert_eq!(name_of(&reise), "Reise");
    let (_, structures) = library.structure().all_elements().await.unwrap();
    let subnet_of = |index: usize| {
        let id = &outcome.items[index].id;
        let entry = structures.iter().find(|s| &s.id == id).unwrap();
        name_of(&entry.subnet_ids[0])
    };
    let by_name: BTreeMap<String, usize> = (0..3).map(|index| (subnet_of(index), index)).collect();
    assert_eq!(by_name.len(), 3);
    let folder_of: Vec<Option<String>> = {
        let mut list = Vec::new();
        for item in &outcome.items {
            list.push(
                media
                    .get(Uuid::try_parse(&item.id).unwrap())
                    .await
                    .unwrap()
                    .import_folder,
            );
        }
        list
    };
    assert!(
        folder_of
            .iter()
            .any(|f| f.as_deref().is_some_and(|f| f.ends_with("Tag 1/Abend")))
    );

    // The same folder again: the subnets are used again, the media too.
    let again = import(media.clone()).await;
    assert_eq!(library.structure().subnets().await.unwrap().len(), 3);
    assert!(again.items.iter().all(|item| !item.created));
    assert_eq!(media_rows(&library).await, 3);
    library.close().await;
}

// --- Trash, uses, thumbnails, structure ------------------------------------------------------

async fn import_one(library: &Library, original: &Original) -> Uuid {
    let media = library.media();
    let preview = media
        .prepare(
            Uuid::new_v4(),
            vec![source(original)],
            Vec::new(),
            None,
            None,
            no_progress(),
        )
        .await
        .unwrap();
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

#[tokio::test]
async fn a_medium_a_document_uses_stays_in_the_trash() {
    let dir = common::temp_dir("pw-media-trash-");
    let original = write_original(&dir.path().join("in"), "Bild.png", &png(400, 200, 13));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let id = import_one(&library, &original).await;
    let id_text = id.hyphenated().to_string();
    // The thumbnail is made on demand (and in the background after import).
    let thumbnail = library.media().thumbnail(id).await.unwrap().unwrap();
    assert!(
        thumbnail
            .path
            .starts_with(path.join("thumbnails/images").canonicalize().unwrap())
    );
    let markdown = format!("# Text\n\n![Bild](pwmedia://{id_text} \"Unterschrift\")\n");
    let document = library
        .documents()
        .create("Text", DocumentFormat::Md, markdown.as_bytes())
        .await
        .unwrap();
    let uses = library.media().uses(id).await.unwrap();
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].document_id, document.id.hyphenated().to_string());

    library
        .structure()
        .set_element_deleted(ElementKind::Media, id, true)
        .await
        .unwrap();
    assert!(matches!(
        library.media().purge(id).await,
        Err(MediaError::InUse { documents: 1 })
    ));
    let report = library.media().empty_trash().await.unwrap();
    assert_eq!((report.removed, report.kept_in_use), (0, 1));
    assert!(path.join(format!("media/images/{id_text}.png")).exists());

    // Without the reference it goes for good, with its thumbnail.
    library
        .documents()
        .save(document.id, b"# Text\n")
        .await
        .unwrap();
    library.media().purge(id).await.unwrap();
    assert!(!path.join(format!("media/images/{id_text}.png")).exists());
    assert!(!thumbnail.path.exists());
    assert!(matches!(
        library.media().get(id).await,
        Err(MediaError::NotFound)
    ));
    library.close().await;
}

#[tokio::test]
async fn pdf_references_count_with_their_page_and_pwrite_lists_its_media() {
    let dir = common::temp_dir("pw-media-uses-");
    let picture = write_original(&dir.path().join("in"), "Skizze.png", &png(10, 10, 14));
    let paper = write_original(&dir.path().join("in"), "Studie.pdf", &pdf("Studie"));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let image = import_one(&library, &picture).await;
    let pdf_id = import_one(&library, &paper).await;
    let content = serde_json::json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [
                { "type": "image", "attrs": { "src": format!("pwmedia://{image}"), "alt": "", "title": "Skizze" } },
                { "type": "text", "text": "siehe S. 12", "marks": [{ "type": "link", "attrs": { "href": format!("pwmedia://{pdf_id}#page=12") } }] },
                { "type": "text", "text": "und gesamt", "marks": [{ "type": "link", "attrs": { "href": format!("pwmedia://{pdf_id}") } }] }
            ]
        }]
    });
    let document = library
        .documents()
        .create_pwrite(
            "Notizen",
            "de-DE",
            serde_json::json!({ "type": "doc", "content": [] }),
        )
        .await
        .unwrap();
    library
        .documents()
        .save_pwrite(document.id, content)
        .await
        .unwrap();
    let pages: Vec<u32> = library
        .media()
        .uses(pdf_id)
        .await
        .unwrap()
        .iter()
        .map(|u| u.page)
        .collect();
    assert_eq!(pages, [0, 12]);
    let (_, stored) = library.documents().read(document.id).await.unwrap();
    let file: serde_json::Value = serde_json::from_slice(&stored).unwrap();
    let entries = file["media"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let entry = entries
        .iter()
        .find(|e| e["id"] == image.hyphenated().to_string())
        .unwrap();
    assert_eq!(entry["sha256"], picture.sha256);
    assert_eq!(entry["originalName"], "Skizze.png");
    assert_eq!(entry["mimeType"], "image/png");
    library.close().await;
}

#[tokio::test]
async fn thumbnails_and_served_files_follow_the_kind() {
    let dir = common::temp_dir("pw-media-thumbs-");
    let svg = write_original(&dir.path().join("in"), "Logo.svg", SVG.as_bytes());
    let paper = write_original(&dir.path().join("in"), "Text.pdf", &pdf("Text"));
    let office = write_original(&dir.path().join("in"), "Brief.docx", &docx(DOCX_MAIN, &[]));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let media = library.media();
    let svg_id = import_one(&library, &svg).await;
    let pdf_id = import_one(&library, &paper).await;
    let office_id = import_one(&library, &office).await;
    assert!(media.thumbnail(svg_id).await.unwrap().is_none());
    assert_eq!(
        media.served_file(svg_id).await.unwrap().mime_type,
        "image/svg+xml"
    );
    assert!(media.thumbnail(pdf_id).await.unwrap().is_none());
    let mut thumb = Vec::new();
    RgbImage::new(200, 260)
        .write_to(&mut Cursor::new(&mut thumb), ImageFormat::Png)
        .unwrap();
    media.store_pdf_thumbnail(pdf_id, &thumb).await.unwrap();
    assert_eq!(
        media.thumbnail(pdf_id).await.unwrap().unwrap().mime_type,
        "image/png"
    );
    assert!(
        media
            .store_pdf_thumbnail(pdf_id, b"not a png")
            .await
            .is_err()
    );
    assert!(media.store_pdf_thumbnail(svg_id, &thumb).await.is_err());
    // Attachments are never served (section 17: nothing is read from them).
    assert!(matches!(
        media.served_file(office_id).await,
        Err(MediaError::NotFound)
    ));
    media.set_page_count(pdf_id, 3).await.unwrap();
    assert_eq!(media.get(pdf_id).await.unwrap().page_count, Some(3));
    assert!(media.set_page_count(svg_id, 3).await.is_err());
    // The copy for the standard program is read-only, the library file stays.
    let copy = media.open_copy(office_id).await.unwrap();
    assert!(copy.ends_with("Brief.docx"));
    assert!(fs::metadata(&copy).unwrap().permissions().readonly());
    assert_eq!(fs::read(&copy).unwrap(), office.bytes);
    library.close().await;
}

#[tokio::test]
async fn media_take_part_in_memberships_tags_and_undo() {
    let dir = common::temp_dir("pw-media-structure-");
    let original = write_original(&dir.path().join("in"), "Karte.png", &png(10, 10, 15));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let id = import_one(&library, &original).await;
    let structure = library.structure();
    let a = structure
        .create_subnet("Atlas", None, None, "")
        .await
        .unwrap();
    let a = Uuid::try_parse(a.subject_id.as_deref().unwrap()).unwrap();
    let b = structure
        .create_subnet("Bilder", None, None, "")
        .await
        .unwrap();
    let b = Uuid::try_parse(b.subject_id.as_deref().unwrap()).unwrap();
    structure
        .assign_element(ElementKind::Media, id, a)
        .await
        .unwrap();
    structure
        .assign_element(ElementKind::Media, id, b)
        .await
        .unwrap();
    structure
        .move_element(ElementKind::Media, id, Some(b))
        .await
        .unwrap();
    structure
        .tag_element(ElementKind::Media, id, "Karten")
        .await
        .unwrap();
    let element =
        project_write_lib::services::structure::Element::media(id.hyphenated().to_string());
    let now = structure.element(&element).await.unwrap();
    assert_eq!(now.subnet_ids, [b.hyphenated().to_string()]);
    assert_eq!(now.tag_ids.len(), 1);
    // One row and one file, wherever it is assigned.
    assert_eq!(media_rows(&library).await, 1);
    assert_eq!(media_files(&path).len(), 1);
    // Undo walks back tag, move and both assignments.
    for _ in 0..4 {
        structure.undo().await.unwrap().unwrap();
    }
    let back = structure.element(&element).await.unwrap();
    assert!(back.subnet_ids.is_empty() && back.tag_ids.is_empty());
    // Deleting a subnet deletes no medium.
    structure
        .assign_element(ElementKind::Media, id, a)
        .await
        .unwrap();
    structure.delete_subnet(a, false).await.unwrap();
    assert!(library.media().get(id).await.is_ok());
    // Trash and archive are journal steps for media too.
    structure
        .set_element_archived(ElementKind::Media, id, true)
        .await
        .unwrap();
    assert!(library.media().get(id).await.unwrap().archived_at.is_some());
    structure.undo().await.unwrap().unwrap();
    assert!(library.media().get(id).await.unwrap().archived_at.is_none());
    library.close().await;
}

#[tokio::test]
async fn the_media_handler_serves_only_images_and_pdfs_with_safe_headers() {
    use project_write_lib::services::media::handler::{CONTENT_SECURITY_POLICY, respond};
    let dir = common::temp_dir("pw-media-handler-");
    let svg = write_original(
        &dir.path().join("in"),
        "Skript.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><script>alert(2)</script></svg>"#,
    );
    let picture = write_original(&dir.path().join("in"), "Foto.png", &png(500, 300, 16));
    let office = write_original(&dir.path().join("in"), "Brief.docx", &docx(DOCX_MAIN, &[]));
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let svg_id = import_one(&library, &svg).await;
    let png_id = import_one(&library, &picture).await;
    let office_id = import_one(&library, &office).await;
    let media = Some(library.media());

    let answer = respond(media.clone(), "GET", &format!("/{svg_id}")).await;
    assert_eq!(answer.status(), 200);
    let headers = answer.headers();
    assert_eq!(headers["content-type"], "image/svg+xml");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["content-security-policy"], CONTENT_SECURITY_POLICY);
    assert!(CONTENT_SECURITY_POLICY.contains("sandbox"));
    assert!(CONTENT_SECURITY_POLICY.contains("default-src 'none'"));
    assert_eq!(answer.body().as_slice(), svg.bytes.as_slice());

    let thumbnail = respond(media.clone(), "GET", &format!("/{png_id}/thumbnail")).await;
    assert_eq!(thumbnail.status(), 200);
    assert_eq!(thumbnail.headers()["content-type"], "image/jpeg");
    let small = image::load_from_memory(thumbnail.body()).unwrap();
    assert_eq!((small.width(), small.height()), (320, 192));

    let head = respond(media.clone(), "HEAD", &format!("/{png_id}")).await;
    assert_eq!(head.status(), 200);
    assert!(head.body().is_empty());
    for (method, request) in [
        ("GET", format!("/{office_id}")),
        ("GET", format!("/{svg_id}/thumbnail")),
        ("GET", format!("/{}", Uuid::new_v4())),
        ("GET", "/../library.json".to_owned()),
        ("GET", format!("/{png_id}/../../database/projectwrite.db")),
    ] {
        let answer = respond(media.clone(), method, &request).await;
        assert_eq!(answer.status(), 404, "{request}");
        assert_eq!(answer.headers()["x-content-type-options"], "nosniff");
    }
    assert_eq!(
        respond(media.clone(), "POST", &format!("/{png_id}"))
            .await
            .status(),
        405
    );
    assert_eq!(
        respond(None, "GET", &format!("/{png_id}")).await.status(),
        404
    );
    library.close().await;
}
