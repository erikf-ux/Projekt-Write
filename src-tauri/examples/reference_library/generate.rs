//! Reproducible reference library (section 6, "Messbare Zielwerte").
//!
//! Everything that varies – ids, time stamps, titles, text – comes from one
//! seed, so the same seed and size always produce the same library. The
//! library is built through the real code paths (`Library::create`, the
//! document repository); nothing is written around them.
//!
//! Phase 2 contains documents only. Media, subnets, tags and confirmed
//! connections join when their tables exist (Phases 7, 8 and 11).
//!
//! Shared by `examples/reference_library/main.rs` and
//! `tests/reference_library.rs`.

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use project_write_lib::clock::{Clock, Generators, IdSource, Timestamp};
use project_write_lib::services::documents::{DocumentFormat, sha256_hex};
use project_write_lib::services::library::Library;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Documents of the full reference library (section 6).
pub const FULL_DOCUMENTS: usize = 10_000;
/// Words of the one very long document (section 6: 50,000 words).
pub const LONG_DOCUMENT_WORDS: usize = 50_000;

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
        serde_json::json!({ "seed": seed, "documents": documents }),
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
    };
    for index in 0..documents {
        let title = title(&mut rng, index);
        let words = target_words(&mut rng, index);
        let (format, content) = if rng.chance(55) {
            summary.markdown += 1;
            (DocumentFormat::Md, markdown(&mut rng, &title, words))
        } else {
            summary.text += 1;
            (DocumentFormat::Txt, plain_text(&mut rng, words))
        };
        summary.bytes += content.len() as u64;
        repository
            .create(&title, format, content.as_bytes())
            .await
            .map_err(|e| format!("document {index}: {e}"))?;
        summary.documents += 1;
        progress(index + 1);
    }
    library.close().await;
    Ok(summary)
}

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
