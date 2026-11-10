//! Reproducible reference library (section 6, "Messbare Zielwerte").
//!
//! Everything that varies – ids, time stamps, titles, text – comes from one
//! seed, so the same seed and size always produce the same library. The
//! library is built through the real code paths (`Library::create`, the
//! document repository); nothing is written around them.
//!
//! Phase 2 built the documents; Phase 7 adds 500 subnets in up to four
//! levels, 1,000 tags, memberships (about 85 % of the documents in one to
//! three subnets) and zero to four tags per document, through the structure
//! service (ADR 0017). Phase 8 adds 5,000 media through the real import
//! (prepare and commit, ADR 0019): pictures in all five formats, PDFs with
//! 1 to 40 pages and DOCX attachments; about one Markdown document in eight shows
//! an image or refers to a PDF page, and about three in four media sit in
//! subnets, some with tags. Confirmed connections join with Phase 11.
//!
//! Shared by `examples/reference_library/main.rs` and
//! `tests/reference_library.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use project_write_lib::clock::{Clock, Generators, IdSource, Timestamp};
use project_write_lib::services::documents::{DocumentFormat, sha256_hex};
use project_write_lib::services::library::Library;
use project_write_lib::services::media::{Decision, SubnetPlacement};
use project_write_lib::services::structure::ElementKind;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Documents of the full reference library (section 6).
pub const FULL_DOCUMENTS: usize = 10_000;
/// Words of the one very long document (section 6: 50,000 words).
pub const LONG_DOCUMENT_WORDS: usize = 50_000;
/// Subnets and tags of the full reference library (section 6).
pub const FULL_SUBNETS: usize = 500;
pub const FULL_TAGS: usize = 1_000;
/// Media of the full reference library (section 6).
pub const FULL_MEDIA: usize = 5_000;
/// Changes whenever the same seed and size give a different library, so a
/// cached copy from an older generator is built again (E2E cache).
pub const GENERATOR_VERSION: u32 = 2;
/// Deepest level of the generated subnet tree.
const MAX_DEPTH: usize = 4;

#[path = "media_files.rs"]
mod media_files;

const MEDIA_WORDS: &[&str] = &[
    "Foto",
    "Skizze",
    "Karte",
    "Scan",
    "Plakat",
    "Grundriss",
    "Porträt",
    "Landschaft",
    "Notizzettel",
    "Tafel",
    "Studie",
    "Bericht",
    "Protokoll",
    "Urkunde",
    "Brief",
];

const SUBNET_WORDS: &[&str] = &[
    "Projekt",
    "Kapitel",
    "Recherche",
    "Figuren",
    "Orte",
    "Notizen",
    "Entwürfe",
    "Quellen",
    "Szenen",
    "Briefe",
    "Archivkisten",
    "Ideen",
    "Wissen",
    "Übersetzungen",
    "Gutachten",
    "Vorträge",
    "Reisen",
    "Interviews",
    "Zeitleisten",
    "Glossar",
];

const TAG_WORDS: &[&str] = &[
    "Entwurf",
    "fertig",
    "prüfen",
    "Zitat",
    "Quelle",
    "Idee",
    "später",
    "wichtig",
    "Österreich",
    "Straße",
    "Geschichte",
    "Technik",
    "Natur",
    "Musik",
    "Kunst",
    "Recht",
    "Medizin",
    "Reise",
    "Familie",
    "Arbeit",
];

/// Subnets and tags for a library of `documents` (the full size has 500 and
/// 1,000; smaller ones keep the proportion).
pub fn structure_size(documents: usize) -> (usize, usize) {
    let scaled = |full: usize| (documents * full / FULL_DOCUMENTS).max(5);
    (scaled(FULL_SUBNETS), scaled(FULL_TAGS))
}

/// Media for a library of `documents` (the full size has 5,000).
pub fn media_size(documents: usize) -> usize {
    (documents * FULL_MEDIA / FULL_DOCUMENTS).max(5)
}

/// Small deterministic pseudo-random generator (SplitMix64).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `low..high` (high exclusive).
    pub fn range(&mut self, low: usize, high: usize) -> usize {
        low + (self.next() % (high - low) as u64) as usize
    }

    pub fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.range(0, items.len())]
    }

    /// `true` with probability `percent`/100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// Ids from the seed (valid version 4 UUIDs).
struct SeededIds(Mutex<Rng>);

impl IdSource for SeededIds {
    fn new_id(&self) -> Uuid {
        let mut rng = self.0.lock().expect("id lock");
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&rng.next().to_le_bytes());
        bytes[8..].copy_from_slice(&rng.next().to_le_bytes());
        uuid::Builder::from_random_bytes(bytes).into_uuid()
    }
}

/// A clock that starts on 2025-01-02 and advances by a varying, seeded step.
struct SeededClock(Mutex<(i64, Rng)>);

impl Clock for SeededClock {
    fn now(&self) -> Timestamp {
        let mut state = self.0.lock().expect("clock lock");
        let step = 1_000 + (state.1.next() % 3_600_000) as i64;
        state.0 += step;
        Timestamp::from_unix_millis(state.0).expect("time within range")
    }
}

/// Deterministic generators for `seed`.
pub fn generators(seed: u64) -> Generators {
    let start = Timestamp::parse_iso("2025-01-02T08:00:00Z")
        .expect("valid start")
        .unix_millis();
    Generators::new(
        Arc::new(SeededClock(Mutex::new((start, Rng::new(seed ^ 0xC10C))))),
        Arc::new(SeededIds(Mutex::new(Rng::new(seed ^ 0x1D5)))),
    )
}

const WORDS: &[&str] = &[
    "der",
    "die",
    "das",
    "und",
    "ist",
    "nicht",
    "mit",
    "auf",
    "für",
    "auch",
    "eine",
    "einen",
    "sich",
    "wie",
    "noch",
    "nach",
    "über",
    "bei",
    "aus",
    "zum",
    "vor",
    "wird",
    "sind",
    "hat",
    "kann",
    "soll",
    "muss",
    "wenn",
    "dann",
    "aber",
    "oder",
    "weil",
    "dass",
    "schon",
    "immer",
    "heute",
    "morgen",
    "gestern",
    "später",
    "früh",
    "hier",
    "dort",
    "sehr",
    "viel",
    "wenig",
    "neue",
    "alte",
    "große",
    "kleine",
    "lange",
    "kurze",
    "gute",
    "klare",
    "offene",
    "schöne",
    "Bibliothek",
    "Dokument",
    "Notiz",
    "Idee",
    "Projekt",
    "Kapitel",
    "Absatz",
    "Seite",
    "Entwurf",
    "Gedanke",
    "Frage",
    "Antwort",
    "Quelle",
    "Zitat",
    "Thema",
    "Beispiel",
    "Krankenhaus",
    "Häuser",
    "Straße",
    "Müller",
    "Schule",
    "Universität",
    "Prüfung",
    "Vorlesung",
    "Hausarbeit",
    "Literatur",
    "Übersicht",
    "Zusammenfassung",
    "Gliederung",
    "Forschung",
    "Ergebnis",
    "Methode",
    "Analyse",
    "Begriff",
    "Definition",
    "Kontext",
    "Wörterbuch",
    "Rechtschreibung",
    "Grammatik",
    "Zeichensetzung",
    "Satz",
    "Wort",
    "Geschichte",
    "Roman",
    "Figur",
    "Handlung",
    "Szene",
    "Dialog",
    "Erzählung",
    "Gedicht",
    "Lehrkraft",
    "Unterricht",
    "Klasse",
    "Aufgabe",
    "Lösung",
    "Material",
    "Arbeitsblatt",
    "Termin",
    "Besprechung",
    "Protokoll",
    "Plan",
    "Ziel",
    "Schritt",
    "Zeitraum",
    "Woche",
    "Garten",
    "Küche",
    "Rezept",
    "Reise",
    "Stadt",
    "Fluss",
    "Berg",
    "Wald",
    "Wetter",
    "schreiben",
    "lesen",
    "ordnen",
    "verknüpfen",
    "prüfen",
    "speichern",
    "finden",
    "denken",
    "erklären",
    "beschreiben",
    "vergleichen",
    "zeigen",
    "beginnen",
    "beenden",
    "ändern",
    "gemeinsam",
    "wichtig",
    "einfach",
    "schwierig",
    "möglich",
    "sicher",
    "lokal",
    "privat",
    "Äpfel",
    "Bäume",
    "Größe",
    "Maß",
    "Fuß",
    "Grüße",
    "Brücke",
    "Zürich",
    "Köln",
    "Österreich",
    "C++",
    "Rust",
    "SQLite",
    "Markdown",
    "E-Mail",
    "z. B.",
    "d. h.",
    "ca.",
    "Dr.",
    "Nr.",
];

const TITLE_WORDS: &[&str] = &[
    "Notizen",
    "Entwurf",
    "Kapitel",
    "Ideen",
    "Projektplan",
    "Protokoll",
    "Zusammenfassung",
    "Literatur",
    "Gliederung",
    "Gedanken",
    "Rezept",
    "Reisebericht",
    "Vorlesung",
    "Hausarbeit",
    "Unterrichtsstunde",
    "Arbeitsblatt",
    "Besprechung",
    "Forschungsfrage",
    "Szene",
    "Gedicht",
    "Übersicht",
    "Prüfungsvorbereitung",
    "Wochenplan",
    "Brief",
    "Antrag",
    "Konzept",
];

const QUALIFIERS: &[&str] = &[
    "zum Roman",
    "für die Klasse 7b",
    "Woche 12",
    "Teil 2",
    "– überarbeitet",
    "März",
    "zur Methode",
    "über Bäume",
    "aus Köln",
    "zur Straße",
    "für Müller",
    "2025",
    "(alt)",
];

fn sentence(rng: &mut Rng) -> String {
    let words = rng.range(5, 19);
    let mut text = String::new();
    for i in 0..words {
        let word = rng.pick(WORDS);
        if i == 0 {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                text.extend(first.to_uppercase());
                text.push_str(chars.as_str());
            }
        } else {
            text.push(' ');
            text.push_str(word);
        }
        if i + 1 < words && rng.chance(8) {
            text.push(',');
        }
    }
    text.push_str(match rng.range(0, 12) {
        0 => "?",
        1 => "!",
        _ => ".",
    });
    text
}

fn paragraph(rng: &mut Rng, words: &mut usize) -> String {
    let mut text = String::new();
    for i in 0..rng.range(2, 8) {
        let s = sentence(rng);
        *words += s.split_whitespace().count();
        if i > 0 {
            text.push(' ');
        }
        text.push_str(&s);
    }
    text
}

/// Plain text with about `target` words.
fn plain_text(rng: &mut Rng, target: usize) -> String {
    let mut words = 0;
    let mut text = String::new();
    while words < target {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&paragraph(rng, &mut words));
    }
    text.push('\n');
    text
}

/// Markdown (CommonMark/GFM) with headings, lists, emphasis and code.
fn markdown(rng: &mut Rng, title: &str, target: usize) -> String {
    let mut words = 0;
    let mut text = format!("# {title}\n");
    while words < target {
        text.push('\n');
        match rng.range(0, 12) {
            0 => {
                let heading = sentence(rng);
                text.push_str(&format!(
                    "## {}\n",
                    heading.trim_end_matches(['.', '?', '!'])
                ));
            }
            1 => {
                for _ in 0..rng.range(2, 6) {
                    let item = sentence(rng);
                    words += item.split_whitespace().count();
                    text.push_str(&format!("- {item}\n"));
                }
            }
            2 => {
                for n in 1..rng.range(3, 6) {
                    let item = sentence(rng);
                    words += item.split_whitespace().count();
                    text.push_str(&format!("{n}. {item}\n"));
                }
            }
            3 => {
                for _ in 0..rng.range(2, 4) {
                    let done = if rng.chance(50) { "x" } else { " " };
                    let item = sentence(rng);
                    words += item.split_whitespace().count();
                    text.push_str(&format!("- [{done}] {item}\n"));
                }
            }
            4 => {
                let quote = paragraph(rng, &mut words);
                text.push_str(&format!("> {quote}\n"));
            }
            5 => {
                text.push_str("```text\n");
                text.push_str(&sentence(rng));
                text.push_str("\n```\n");
            }
            _ => {
                let mut para = paragraph(rng, &mut words);
                if rng.chance(40)
                    && let Some(space) = para.find(' ')
                {
                    let end = para[space + 1..]
                        .find(' ')
                        .map_or(para.len(), |e| space + 1 + e);
                    let word = para[space + 1..end].to_owned();
                    let style = if rng.chance(50) { "**" } else { "_" };
                    para.replace_range(space + 1..end, &format!("{style}{word}{style}"));
                }
                if rng.chance(15) {
                    para.push_str(" Siehe `projectwrite.db`.");
                }
                text.push_str(&para);
                text.push('\n');
            }
        }
    }
    text
}

fn title(rng: &mut Rng, index: usize) -> String {
    let base = rng.pick(TITLE_WORDS);
    if rng.chance(60) {
        format!("{base} {}", rng.pick(QUALIFIERS))
    } else {
        format!("{base} {}", index + 1)
    }
}

/// Size class of the document at `index`, in words.
fn target_words(rng: &mut Rng, index: usize) -> usize {
    if index == 0 {
        return LONG_DOCUMENT_WORDS;
    }
    match rng.range(0, 100) {
        0..70 => rng.range(80, 600),
        70..95 => rng.range(600, 3_000),
        _ => rng.range(3_000, 10_000),
    }
}

/// Totals of a generated library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub documents: usize,
    pub markdown: usize,
    pub text: usize,
    pub bytes: u64,
    pub subnets: usize,
    pub tags: usize,
    pub memberships: usize,
    pub document_tags: usize,
    pub unassigned: usize,
    pub media: usize,
    pub images: usize,
    pub pdfs: usize,
    pub attachments: usize,
    pub media_bytes: u64,
    /// Rows of `document_media`: images shown and PDF pages referred to.
    pub media_uses: usize,
    pub media_memberships: usize,
    pub media_tags: usize,
    pub media_unassigned: usize,
}

/// Creates the reference library in `path` (missing or empty folder).
pub async fn generate(
    path: &Path,
    seed: u64,
    documents: usize,
    mut progress: impl FnMut(usize),
) -> Result<Summary, String> {
    let (library, _) = Library::create(path, generators(seed))
        .await
        .map_err(|e| format!("cannot create the library: {e}"))?;
    // Mark the folder, so the script may replace it later (--force).
    let mut manifest = library.manifest().clone();
    manifest.other.insert(
        "referenceLibrary".to_owned(),
        serde_json::json!({
            "seed": seed,
            "documents": documents,
            "media": media_size(documents),
            "generator": GENERATOR_VERSION,
        }),
    );
    manifest
        .write(library.root())
        .map_err(|e| format!("cannot mark the library: {e}"))?;

    let repository = library.documents();
    let mut rng = Rng::new(seed);
    let mut summary = Summary {
        documents: 0,
        markdown: 0,
        text: 0,
        bytes: 0,
        subnets: 0,
        tags: 0,
        memberships: 0,
        document_tags: 0,
        unassigned: 0,
        media: 0,
        images: 0,
        pdfs: 0,
        attachments: 0,
        media_bytes: 0,
        media_uses: 0,
        media_memberships: 0,
        media_tags: 0,
        media_unassigned: 0,
    };
    // Media first: documents refer to them, and a reference needs its row.
    let media = build_media(&library, seed, media_size(documents), &mut summary).await?;
    let images: Vec<&Medium> = media
        .iter()
        .filter(|m| m.kind == MediaKind::Image)
        .collect();
    let pdfs: Vec<&Medium> = media.iter().filter(|m| m.kind == MediaKind::Pdf).collect();
    let mut references = Rng::new(seed ^ 0x3EF5);
    let mut created = Vec::with_capacity(documents);
    for index in 0..documents {
        let title = title(&mut rng, index);
        let words = target_words(&mut rng, index);
        let (format, content) = if rng.chance(55) {
            summary.markdown += 1;
            let mut text = markdown(&mut rng, &title, words);
            summary.media_uses += refer_to_media(&mut references, &mut text, &images, &pdfs);
            (DocumentFormat::Md, text)
        } else {
            summary.text += 1;
            (DocumentFormat::Txt, plain_text(&mut rng, words))
        };
        summary.bytes += content.len() as u64;
        let meta = repository
            .create(&title, format, content.as_bytes())
            .await
            .map_err(|e| format!("document {index}: {e}"))?;
        created.push(meta.id);
        summary.documents += 1;
        progress(index + 1);
    }
    let (subnets, tags) = build_structure(&library, seed, &created, &mut summary).await?;
    place_media(&library, seed, &media, &subnets, &tags, &mut summary).await?;
    library.close().await;
    Ok(summary)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    Image,
    Pdf,
    Attachment,
}

/// A medium of the library as documents refer to it.
struct Medium {
    id: Uuid,
    kind: MediaKind,
    title: String,
    pages: usize,
}

/// Imports the media one by one through prepare and commit, as a file
/// pasted into the window would come (ADR 0019, decisions 5 and 6).
async fn build_media(
    library: &Library,
    seed: u64,
    count: usize,
    summary: &mut Summary,
) -> Result<Vec<Medium>, String> {
    let service = library.media();
    let mut rng = Rng::new(seed ^ 0x3ED1A);
    let mut created = Vec::with_capacity(count);
    let started = std::time::Instant::now();
    for index in 0..count {
        if index > 0 && index % 1000 == 0 {
            eprintln!("  {index} / {count} media");
        }
        let word = rng.pick(MEDIA_WORDS);
        let title = format!("{word} {:04}", index + 1);
        let roll = rng.range(0, 100);
        let (name, bytes, kind, pages) = if roll < 72 {
            // Pictures up to 640 × 480: the larger ones get a thumbnail.
            let width = rng.range(48, 641) as u32;
            let height = rng.range(48, 481) as u32;
            let (ending, bytes) = match roll {
                0..30 => ("png", media_files::png(&mut rng, width, height)),
                30..52 => ("jpg", media_files::jpeg(&mut rng, width, height)),
                52..58 => ("gif", media_files::gif(&mut rng, width, height)),
                58..65 => ("webp", media_files::webp(&mut rng, width, height)),
                _ => ("svg", media_files::svg(&mut rng, width, height, &title)),
            };
            (format!("{title}.{ending}"), bytes, MediaKind::Image, 0)
        } else if roll < 92 {
            let pages = rng.range(1, 41);
            let lines: Vec<String> = (0..8).map(|_| sentence(&mut rng)).collect();
            let bytes = media_files::pdf(&title, pages, &lines);
            (format!("{title}.pdf"), bytes, MediaKind::Pdf, pages)
        } else {
            let lines: Vec<String> = (0..rng.range(3, 30)).map(|_| sentence(&mut rng)).collect();
            (
                format!("{title}.docx"),
                media_files::docx(&lines),
                MediaKind::Attachment,
                0,
            )
        };
        let preview = service
            .prepare_bytes(&name, &bytes)
            .await
            .map_err(|e| format!("medium {index} ({name}): {e}"))?;
        if preview.items.len() != 1 {
            return Err(format!(
                "medium {index} ({name}) was refused: {:?}",
                preview.skipped
            ));
        }
        let batch = Uuid::try_parse(&preview.batch_id).map_err(|e| e.to_string())?;
        let outcome = service
            .commit_import(
                batch,
                &BTreeMap::new(),
                Decision::Import,
                &SubnetPlacement::default(),
            )
            .await
            .map_err(|e| format!("medium {index} ({name}): {e}"))?;
        let id = outcome
            .items
            .first()
            .and_then(|item| Uuid::try_parse(&item.id).ok())
            .ok_or_else(|| format!("medium {index} ({name}): no id"))?;
        if kind == MediaKind::Pdf {
            // The PDF view reports the page count after the first look.
            service
                .set_page_count(id, pages as u32)
                .await
                .map_err(|e| format!("pages of medium {index}: {e}"))?;
        }
        match kind {
            MediaKind::Image => summary.images += 1,
            MediaKind::Pdf => summary.pdfs += 1,
            MediaKind::Attachment => summary.attachments += 1,
        }
        summary.media += 1;
        summary.media_bytes += bytes.len() as u64;
        created.push(Medium {
            id,
            kind,
            title,
            pages,
        });
    }
    eprintln!(
        "  {count} / {count} media ({:.1} s)",
        started.elapsed().as_secs_f64()
    );
    Ok(created)
}

/// About one Markdown document in eight shows an image, about one in ten
/// refers to a PDF page. Returns the new `document_media` rows.
fn refer_to_media(rng: &mut Rng, text: &mut String, images: &[&Medium], pdfs: &[&Medium]) -> usize {
    let mut uses = BTreeSet::new();
    if !images.is_empty() && rng.chance(12) {
        let image = images[rng.range(0, images.len())];
        let caption = if rng.chance(50) {
            format!(" \"{}\"", image.title)
        } else {
            String::new()
        };
        text.push_str(&format!(
            "\n![Abbildung: {}](pwmedia://{}{caption})\n",
            image.title,
            image.id.hyphenated()
        ));
        uses.insert((image.id, 0));
    }
    if !pdfs.is_empty() && rng.chance(10) {
        let pdf = pdfs[rng.range(0, pdfs.len())];
        let page = rng.range(1, pdf.pages + 1);
        text.push_str(&format!(
            "\nSiehe [{}, Seite {page}](pwmedia://{}#page={page}).\n",
            pdf.title,
            pdf.id.hyphenated()
        ));
        uses.insert((pdf.id, page));
    }
    uses.len()
}

/// Subnets and tags for media (Phase 8): about three in four media in one
/// or two subnets, zero to two tags each; every 500th is a favourite (a
/// handful, as in real use – "Favoriten" stands open at the top).
async fn place_media(
    library: &Library,
    seed: u64,
    media: &[Medium],
    subnets: &[Uuid],
    tags: &[String],
    summary: &mut Summary,
) -> Result<(), String> {
    let structure = library.structure();
    let service = library.media();
    let mut rng = Rng::new(seed ^ 0x9A1E);
    for (index, medium) in media.iter().enumerate() {
        if subnets.is_empty() || rng.chance(25) {
            summary.media_unassigned += 1;
        } else {
            for _ in 0..rng.range(1, 3) {
                let subnet = subnets[rng.range(0, subnets.len())];
                let outcome = structure
                    .assign_element(ElementKind::Media, medium.id, subnet)
                    .await
                    .map_err(|e| format!("membership of medium {index}: {e}"))?;
                if outcome.step.is_some() {
                    summary.media_memberships += 1;
                }
            }
        }
        if !tags.is_empty() {
            for _ in 0..rng.range(0, 3) {
                let name = &tags[rng.range(0, tags.len())];
                let outcome = structure
                    .tag_element(ElementKind::Media, medium.id, name)
                    .await
                    .map_err(|e| format!("tag of medium {index}: {e}"))?;
                if outcome.step.is_some() {
                    summary.media_tags += 1;
                }
            }
        }
        if index % 500 == 0 {
            service
                .set_favorite(medium.id, true)
                .await
                .map_err(|e| format!("favourite medium {index}: {e}"))?;
        }
    }
    Ok(())
}

/// Subnets, tags, memberships and document tags (Phase 7). Returns the
/// subnets and the tag names for the media.
async fn build_structure(
    library: &Library,
    seed: u64,
    documents: &[Uuid],
    summary: &mut Summary,
) -> Result<(Vec<Uuid>, Vec<String>), String> {
    let structure = library.structure();
    let mut rng = Rng::new(seed ^ 0x5B7E);
    let (subnet_count, tag_count) = structure_size(documents.len());
    // One in 25 on the top level, the rest below a random earlier subnet that
    // is not yet at the deepest level.
    let roots = (subnet_count / 25).max(1);
    let mut subnets: Vec<(Uuid, usize)> = Vec::with_capacity(subnet_count);
    for index in 0..subnet_count {
        let (parent, depth) = if index < roots {
            (None, 1)
        } else {
            let candidates: Vec<&(Uuid, usize)> = subnets
                .iter()
                .filter(|(_, depth)| *depth < MAX_DEPTH)
                .collect();
            let (id, depth) = candidates[rng.range(0, candidates.len())];
            (Some(*id), depth + 1)
        };
        let name = format!("{} {}", rng.pick(SUBNET_WORDS), index + 1);
        // Every seventh subnet chooses its colour; the others inherit it.
        let color =
            (index % 7 == 0).then(|| ["blue", "green", "violet", "orange", "grey"][index % 5]);
        let description = if rng.chance(30) {
            sentence(&mut rng)
        } else {
            String::new()
        };
        let outcome = structure
            .create_subnet(&name, parent, color, &description)
            .await
            .map_err(|e| format!("subnet {index}: {e}"))?;
        let id = outcome
            .subject_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| format!("subnet {index}: no id"))?;
        subnets.push((id, depth));
        summary.subnets += 1;
    }
    let tags: Vec<String> = (0..tag_count)
        .map(|index| format!("{} {}", rng.pick(TAG_WORDS), index + 1))
        .collect();
    for name in &tags {
        structure
            .create_tag(name)
            .await
            .map_err(|e| format!("tag {name}: {e}"))?;
        summary.tags += 1;
    }
    for (index, document) in documents.iter().enumerate() {
        // About 15 % stay "Nicht zugeordnet".
        if rng.chance(15) {
            summary.unassigned += 1;
        } else {
            for _ in 0..rng.range(1, 4) {
                let (subnet, _) = subnets[rng.range(0, subnets.len())];
                let outcome = structure
                    .assign(*document, subnet)
                    .await
                    .map_err(|e| format!("membership of document {index}: {e}"))?;
                if outcome.step.is_some() {
                    summary.memberships += 1;
                }
            }
        }
        for _ in 0..rng.range(0, 5) {
            let name = &tags[rng.range(0, tags.len())];
            let outcome = structure
                .tag_document(*document, name)
                .await
                .map_err(|e| format!("tag of document {index}: {e}"))?;
            if outcome.step.is_some() {
                summary.document_tags += 1;
            }
        }
    }
    Ok((subnets.into_iter().map(|(id, _)| id).collect(), tags))
}

/// A subnet row as it goes into the fingerprint: id, name, description,
/// colour, parent, created, updated.
type SubnetFingerprint = (
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
);

/// A membership row: subnet, document or medium, primary, created.
type MembershipFingerprint = (String, Option<String>, Option<String>, i64, String);

/// A media row as it goes into the fingerprint, in two parts (tuples of
/// more than twelve have no `Debug`): id, kind, type, extension, title,
/// description, original name; id, size, SHA-256, width, height, pages,
/// created, updated, favourite.
type MediaFingerprint = (String, String, String, String, String, String, String);
type MediaFileFingerprint = (
    String,
    i64,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    Option<String>,
);

/// SHA-256 over everything that defines the library's content: every file
/// except `library.lock` (process id) and the database files (compared
/// through their rows instead), plus all rows and the schema version.
pub async fn fingerprint(path: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect(path, path, &mut files).map_err(|e| e.to_string())?;
    files.sort();
    let mut hasher = Sha256::new();
    for (relative, digest) in &files {
        hasher.update(format!("file {relative} {digest}\n"));
    }

    let database = project_write_lib::database::Database::open_existing(
        &path.join("database").join("projectwrite.db"),
        project_write_lib::database::JournalMode::Rollback,
    )
    .await
    .map_err(|e| e.to_string())?;
    hasher.update(format!(
        "schema {}\n",
        database.schema_version().await.map_err(|e| e.to_string())?
    ));
    let documents: Vec<(String, String, String, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, title, format, size_bytes, content_sha256, created_at, updated_at FROM documents ORDER BY id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &documents {
        hasher.update(format!("document {row:?}\n"));
    }
    let settings: Vec<(String, String, String)> =
        sqlx::query_as("SELECT key, value, updated_at FROM application_settings ORDER BY key")
            .fetch_all(database.reader())
            .await
            .map_err(|e| e.to_string())?;
    for row in &settings {
        hasher.update(format!("setting {row:?}\n"));
    }
    // Phase 7: subnets, memberships, tags.
    let subnets: Vec<SubnetFingerprint> =
        sqlx::query_as(
            "SELECT id, name, description, color, parent_id, created_at, updated_at FROM subnets ORDER BY id",
        )
        .fetch_all(database.reader())
        .await
        .map_err(|e| e.to_string())?;
    for row in &subnets {
        hasher.update(format!("subnet {row:?}\n"));
    }
    // Phase 8: a membership belongs to a document or a medium.
    let memberships: Vec<MembershipFingerprint> = sqlx::query_as(
        "SELECT subnet_id, document_id, media_id, is_primary, created_at FROM subnet_memberships \
         ORDER BY document_id, media_id, subnet_id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &memberships {
        hasher.update(format!("membership {row:?}\n"));
    }
    let tags: Vec<(String, String, String, String)> =
        sqlx::query_as("SELECT id, name, created_at, updated_at FROM tags ORDER BY id")
            .fetch_all(database.reader())
            .await
            .map_err(|e| e.to_string())?;
    for row in &tags {
        hasher.update(format!("tag {row:?}\n"));
    }
    let document_tags: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT document_id, tag_id, created_at FROM document_tags ORDER BY document_id, tag_id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &document_tags {
        hasher.update(format!("document tag {row:?}\n"));
    }
    // Phase 8: media, their uses in documents and their tags.
    let media: Vec<MediaFingerprint> = sqlx::query_as(
        "SELECT id, kind, mime_type, extension, title, description, original_name FROM media ORDER BY id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &media {
        hasher.update(format!("medium {row:?}\n"));
    }
    let media_files: Vec<MediaFileFingerprint> = sqlx::query_as(
        "SELECT id, size_bytes, sha256, width, height, page_count, created_at, updated_at, favorited_at \
         FROM media ORDER BY id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &media_files {
        hasher.update(format!("medium file {row:?}\n"));
    }
    let uses: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT document_id, media_id, page FROM document_media ORDER BY document_id, media_id, page",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &uses {
        hasher.update(format!("medium use {row:?}\n"));
    }
    let media_tags: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT media_id, tag_id, created_at FROM media_tags ORDER BY media_id, tag_id",
    )
    .fetch_all(database.reader())
    .await
    .map_err(|e| e.to_string())?;
    for row in &media_tags {
        hasher.update(format!("medium tag {row:?}\n"));
    }
    database.close().await;
    Ok(sha256_hex(&hasher.finalize()))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("inside root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "library.lock" || relative == "database" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect(root, &path, out)?;
        } else {
            out.push((relative, sha256_hex(&std::fs::read(&path)?)));
        }
    }
    Ok(())
}
