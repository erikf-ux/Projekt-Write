//! Exports (Phase 10, section 21, ADR 0021): one document as Markdown with
//! its media, the whole library as a folder that is usable without the app.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use project_write_lib::clock::Generators;
use project_write_lib::services::documents::DocumentFormat;
use project_write_lib::services::export::{ExportError, TextKind};
use project_write_lib::services::library::Library;
use project_write_lib::services::media::{Decision, SubnetPlacement};
use project_write_lib::services::pwrite;
use uuid::Uuid;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#3a7"/></svg>"##;

async fn create(path: &Path) -> Library {
    Library::create(path, Generators::system()).await.unwrap().0
}

async fn import_svg(library: &Library, name: &str) -> Uuid {
    let media = library.media();
    let preview = media
        .prepare_bytes(name, format!("{SVG}<!-- {name} -->").as_bytes())
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

/// Resolves a percent-encoded relative link from the folder of `file`.
fn follow(file: &Path, link: &str) -> std::path::PathBuf {
    let mut decoded = Vec::new();
    let bytes = link.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            decoded.push(u8::from_str_radix(&link[index + 1..index + 3], 16).unwrap());
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let path = String::from_utf8(decoded).unwrap();
    let path = path.split('#').next().unwrap();
    let mut target = file.parent().unwrap().to_path_buf();
    for part in path.split('/') {
        if part == ".." {
            target.pop();
        } else {
            target.push(part);
        }
    }
    target
}

fn links_in(markdown: &str) -> Vec<String> {
    markdown
        .split("](")
        .skip(1)
        .map(|rest| rest.split(')').next().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_document_exports_as_markdown_with_its_media() {
    let dir = common::temp_dir("pw-export-");
    let library = create(&dir.path().join("lib")).await;
    let image = import_svg(&library, "Skizze vom Hafen.svg").await;
    let other = library
        .documents()
        .create("Anderes", DocumentFormat::Md, b"x")
        .await
        .unwrap();
    let text = format!(
        "# Hafen\n\n![Skizze](pwmedia://{image})\n\nSiehe [das Andere](pwrite://{}) und ![Fehlt](pwmedia://{}).\n",
        other.id,
        Uuid::new_v4()
    );
    let doc = library
        .documents()
        .create("Hafen: Notizen", DocumentFormat::Md, text.as_bytes())
        .await
        .unwrap();
    let exports = library.exports();
    let name = exports
        .suggested_name(doc.id, TextKind::Markdown)
        .await
        .unwrap();
    assert_eq!(name, "Hafen_ Notizen.md");
    let out = dir.path().join("ausgabe");
    fs::create_dir_all(&out).unwrap();
    let file = out.join(&name);
    let outcome = exports
        .export_document(doc.id, &text, TextKind::Markdown, &file)
        .await
        .unwrap();
    assert_eq!(outcome.media, 1);
    let written = fs::read_to_string(&file).unwrap();
    assert!(written.contains("Siehe das Andere und"), "{written}");
    assert!(!written.contains("pwrite://"), "{written}");
    let links = links_in(&written);
    let copied = follow(&file, &links[0]);
    assert_eq!(
        copied,
        out.join("Hafen_ Notizen-Medien")
            .join("Skizze vom Hafen.svg")
    );
    assert_eq!(
        fs::read(&copied).unwrap(),
        format!("{SVG}<!-- Skizze vom Hafen.svg -->").as_bytes()
    );

    // TXT: the text as it is.
    let txt = out.join("Hafen.txt");
    exports
        .export_document(doc.id, "Nur Text\n", TextKind::Txt, &txt)
        .await
        .unwrap();
    assert_eq!(fs::read_to_string(&txt).unwrap(), "Nur Text\n");

    // Inside the library only exports/ is a target.
    let inside = dir.path().join("lib/documents/x.md");
    assert!(matches!(
        exports
            .export_document(doc.id, "x", TextKind::Txt, &inside)
            .await,
        Err(ExportError::TargetInsideLibrary)
    ));
    library.close().await;
}

#[tokio::test]
async fn the_library_exports_as_a_usable_folder() {
    let dir = common::temp_dir("pw-export-");
    let library = create(&dir.path().join("lib")).await;
    let documents = library.documents();
    let structure = library.structure();
    let image = import_svg(&library, "plan.svg").await;
    let travel = structure
        .create_subnet("Reise", None, None, "")
        .await
        .unwrap()
        .subject_id
        .unwrap();
    let travel = Uuid::try_parse(&travel).unwrap();
    let day = structure
        .create_subnet("Tag 1: Ankunft", Some(travel), None, "")
        .await
        .unwrap()
        .subject_id
        .unwrap();
    let day = Uuid::try_parse(&day).unwrap();

    let top = documents
        .create("Übersicht", DocumentFormat::Md, b"Start")
        .await
        .unwrap();
    let rich = pwrite::new_file(
        &Uuid::nil().to_string(),
        "Tagebuch",
        "2026-11-11T10:00:00.000Z",
        "de",
        serde_json::json!({"type": "doc", "content": []}),
    );
    let diary = documents
        .create("Tagebuch", DocumentFormat::Pwrite, &pwrite::to_bytes(&rich))
        .await
        .unwrap();
    structure.assign(diary.id, day).await.unwrap();
    let note = documents
        .create(
            "Notiz",
            DocumentFormat::Md,
            format!(
                "Zum [Tagebuch](pwrite://{}) mit ![Plan](pwmedia://{image})\n",
                diary.id
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    structure.assign(note.id, travel).await.unwrap();
    let _plain = documents
        .create("Liste", DocumentFormat::Txt, b"eins\nzwei\n")
        .await
        .unwrap();
    let old = documents
        .create("Alt", DocumentFormat::Md, b"vorbei")
        .await
        .unwrap();
    documents.set_archived(old.id, true).await.unwrap();
    let gone = documents
        .create("Weg", DocumentFormat::Md, b"geloescht")
        .await
        .unwrap();
    documents.move_to_trash(gone.id).await.unwrap();
    let _ = top;

    let parent = dir.path().join("exporte");
    fs::create_dir_all(&parent).unwrap();
    let exports = library.exports();
    let plan = exports.begin_library_export(&parent).await.unwrap();
    assert_eq!(plan.written, 4, "{plan:?}");
    assert_eq!(plan.to_convert.len(), 1);
    assert_eq!(plan.to_convert[0].id, diary.id.to_string());
    let export_id = Uuid::try_parse(&plan.export_id).unwrap();
    // Nothing looks finished yet.
    let visible: Vec<String> = fs::read_dir(&parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .collect();
    assert!(visible.is_empty(), "{visible:?}");
    exports
        .add_converted(
            export_id,
            diary.id,
            &format!(
                "# Tagebuch\n\n![Plan](pwmedia://{image}) und [Notiz](pwrite://{})\n",
                note.id
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        exports.add_converted(export_id, diary.id, "nochmal").await,
        Err(ExportError::NotFound)
    ));
    let outcome = exports.finish_library_export(export_id).await.unwrap();
    assert_eq!(outcome.documents, 5);
    assert_eq!(outcome.media, 1);

    let root = fs::read_dir(&parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("Project Write Export")
        })
        .expect("the export folder");
    assert!(root.join("Übersicht.md").is_file());
    assert_eq!(
        fs::read_to_string(root.join("Liste.txt")).unwrap(),
        "eins\nzwei\n"
    );
    assert!(root.join("Archiv/Alt.md").is_file());
    assert!(!root.join("Weg.md").exists());
    let note_file = root.join("Reise/Notiz.md");
    let diary_file = root.join("Reise/Tag 1_ Ankunft/Tagebuch.md");
    assert!(diary_file.is_file());
    // Links lead to the exported files.
    let note_text = fs::read_to_string(&note_file).unwrap();
    let links = links_in(&note_text);
    assert_eq!(follow(&note_file, &links[0]), diary_file);
    assert_eq!(follow(&note_file, &links[1]), root.join("Medien/plan.svg"));
    let diary_text = fs::read_to_string(&diary_file).unwrap();
    let links = links_in(&diary_text);
    assert_eq!(follow(&diary_file, &links[0]), root.join("Medien/plan.svg"));
    assert_eq!(follow(&diary_file, &links[1]), note_file);
    let index = fs::read_to_string(root.join("index.md")).unwrap();
    for link in links_in(&index) {
        assert!(follow(&root.join("index.md"), &link).is_file(), "{link}");
    }

    // A cancelled export leaves nothing.
    let plan = exports.begin_library_export(&parent).await.unwrap();
    assert!(exports.cancel_library_export(Uuid::try_parse(&plan.export_id).unwrap()));
    let hidden: Vec<String> = fs::read_dir(&parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(hidden.is_empty(), "{hidden:?}");
    assert!(matches!(
        exports
            .begin_library_export(&dir.path().join("lib/media"))
            .await,
        Err(ExportError::TargetInsideLibrary) | Err(ExportError::Path(_)) | Err(ExportError::Io(_))
    ));
    library.close().await;
}

/// A change to the entries of a ZIP (name and bytes).
type Change = dyn Fn(&mut Vec<(String, Vec<u8>)>);

fn rezip(path: &Path, out: &Path, change: impl Fn(&mut Vec<(String, Vec<u8>)>)) {
    use std::io::{Read, Write};
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (entry.name().unwrap().into_owned(), data)
        })
        .collect();
    change(&mut entries);
    let mut writer = zip::ZipWriter::new(fs::File::create(out).unwrap());
    for (name, data) in &entries {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}

async fn counts(library: &Library) -> (i64, i64) {
    let pool = library.database().reader();
    let documents: i64 = sqlx::query_scalar("SELECT count(*) FROM documents")
        .fetch_one(pool)
        .await
        .unwrap();
    let media: i64 = sqlx::query_scalar("SELECT count(*) FROM media")
        .fetch_one(pool)
        .await
        .unwrap();
    (documents, media)
}

#[tokio::test]
async fn a_package_travels_with_its_media_and_a_changed_one_is_refused() {
    use project_write_lib::services::export::pack;

    let dir = common::temp_dir("pw-export-");
    let source = create(&dir.path().join("quelle")).await;
    let image = import_svg(&source, "karte.svg").await;
    let file = pwrite::new_file(
        &Uuid::nil().to_string(),
        "Reisebericht",
        "2026-11-11T10:00:00.000Z",
        "de",
        serde_json::json!({"type": "doc", "content": [
            {"type": "paragraph", "content": [{"type": "text", "text": "Mit Karte"}]},
            {"type": "image", "attrs": {"src": format!("pwmedia://{image}"), "alt": "Karte"}}
        ]}),
    );
    let bytes = pwrite::to_bytes(&file);
    let out = dir.path().join("pakete");
    fs::create_dir_all(&out).unwrap();
    let package = out.join("Reisebericht.pwritepack");
    assert_eq!(
        source
            .exports()
            .export_pack(&bytes, &package)
            .await
            .unwrap(),
        1
    );
    assert!(!out.join("Reisebericht.pwritepack.part").exists());
    source.close().await;

    let target = create(&dir.path().join("ziel")).await;
    let imported = pack::import_pack(&package, &target.media(), &target.documents())
        .await
        .unwrap();
    assert_eq!((imported.imported, imported.reused), (1, 0));
    assert_eq!(imported.title, "Reisebericht");
    let new_id = Uuid::try_parse(&imported.document_id).unwrap();
    let media = target.media().list().await.unwrap();
    assert_eq!(media.len(), 1);
    let (_, content) = target.documents().read(new_id).await.unwrap();
    let text = String::from_utf8(content).unwrap();
    assert!(
        text.contains(&format!("pwmedia://{}", media[0].id)),
        "{text}"
    );
    assert!(!text.contains(&image.to_string()));
    // Again: the medium is used, a second document comes.
    let again = pack::import_pack(&package, &target.media(), &target.documents())
        .await
        .unwrap();
    assert_eq!((again.imported, again.reused), (0, 1));
    assert_eq!(counts(&target).await, (2, 1));

    let medium_entry = format!("media/{image}.svg");
    let cases: Vec<(&str, Box<Change>)> = vec![
        (
            "changed medium",
            Box::new(|entries: &mut Vec<(String, Vec<u8>)>| {
                entries
                    .iter_mut()
                    .find(|(name, _)| name.starts_with("media/"))
                    .unwrap()
                    .1
                    .push(b' ');
            }),
        ),
        (
            "changed document",
            Box::new(|entries: &mut Vec<(String, Vec<u8>)>| {
                entries
                    .iter_mut()
                    .find(|(name, _)| name == "document.pwrite")
                    .unwrap()
                    .1
                    .push(b' ');
            }),
        ),
        (
            "extra entry",
            Box::new(|entries: &mut Vec<(String, Vec<u8>)>| {
                entries.push((format!("media/{}.png", Uuid::new_v4()), b"x".to_vec()));
            }),
        ),
        (
            "path out",
            Box::new(|entries: &mut Vec<(String, Vec<u8>)>| {
                entries.push(("../boese.pwrite".to_owned(), b"x".to_vec()));
            }),
        ),
        (
            "no manifest",
            Box::new(|entries: &mut Vec<(String, Vec<u8>)>| {
                entries.retain(|(name, _)| name != "manifest.json");
            }),
        ),
    ];
    for (index, (what, change)) in cases.iter().enumerate() {
        let changed = out.join(format!("fall-{index}.pwritepack"));
        rezip(&package, &changed, change);
        let result = pack::import_pack(&changed, &target.media(), &target.documents()).await;
        assert!(result.is_err(), "{what} was imported");
        assert_eq!(counts(&target).await, (2, 1), "{what} left something");
    }
    // A program disguised as a picture, checksums matching: the type check
    // refuses it and nothing stays.
    let disguised = out.join("getarnt.pwritepack");
    rezip(&package, &disguised, |entries| {
        let program = b"MZ\x90\x00 kein Bild".to_vec();
        let sha = project_write_lib::services::documents::sha256_hex(&program);
        let size = program.len();
        entries
            .iter_mut()
            .find(|(name, _)| name.starts_with("media/"))
            .unwrap()
            .1 = program;
        let manifest = entries
            .iter_mut()
            .find(|(name, _)| name == "manifest.json")
            .unwrap();
        let mut json: serde_json::Value = serde_json::from_slice(&manifest.1).unwrap();
        json["media"][0]["sha256"] = serde_json::Value::from(sha);
        json["media"][0]["size"] = serde_json::Value::from(size);
        manifest.1 = serde_json::to_vec(&json).unwrap();
    });
    assert!(
        pack::import_pack(&disguised, &target.media(), &target.documents())
            .await
            .is_err()
    );
    assert_eq!(counts(&target).await, (2, 1));
    let _ = medium_entry;
    let tmp_left: Vec<_> = fs::read_dir(dir.path().join("ziel/tmp"))
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(tmp_left.is_empty(), "{tmp_left:?}");
    target.close().await;
}
