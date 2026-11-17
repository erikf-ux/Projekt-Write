//! Acceptance of phase 12 with the German corpus (ADR 0023):
//!
//! - at most 3 % of the correct words are flagged, at least 95 % of the
//!   errors are found;
//! - the autocorrect changes no correct word and corrects no error wrongly;
//! - the shipped dictionaries are the files named in `SHA256SUMS`.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use project_write_lib::services::language::autocorrect::{Blocked, Candidate, Span};
use project_write_lib::services::language::dictionaries::DictionaryStore;
use project_write_lib::services::language::engine::{FindingKind, OwnWords};
use project_write_lib::services::language::{CheckParts, LanguageService};
use sha2::{Digest, Sha256};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn service() -> LanguageService {
    LanguageService::new(Arc::new(DictionaryStore::new(
        Some(root().join("dictionaries")),
        root().join("target/no-added-dictionaries"),
    )))
}

fn lines(name: &str) -> Vec<String> {
    fs::read_to_string(root().join("tests/data/language").join(name))
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn errors() -> Vec<(String, String)> {
    lines("de-fehler.txt")
        .into_iter()
        .map(|line| {
            let mut parts = line.split('\t');
            (
                parts.next().unwrap().to_owned(),
                parts.next().unwrap().to_owned(),
            )
        })
        .collect()
}

const SPELLING: CheckParts = CheckParts {
    spelling: true,
    rules: false,
};

fn flagged(service: &LanguageService, word: &str) -> bool {
    service
        .check(word, "de-DE", &[], &OwnWords::default(), SPELLING, None)
        .findings
        .iter()
        .any(|f| f.kind == FindingKind::Spelling)
}

#[test]
fn the_corpus_meets_the_acceptance_of_phase_12() {
    let service = service();
    let correct = lines("de-richtig.txt");
    let wrongly_flagged: Vec<&String> = correct.iter().filter(|w| flagged(&service, w)).collect();
    let errors = errors();
    let missed: Vec<&String> = errors
        .iter()
        .map(|(error, _)| error)
        .filter(|error| !flagged(&service, error))
        .collect();
    let flagged_share = wrongly_flagged.len() as f64 / correct.len() as f64;
    let found_share = 1.0 - missed.len() as f64 / errors.len() as f64;
    println!(
        "correct words flagged: {} of {} ({:.1} %) {wrongly_flagged:?}",
        wrongly_flagged.len(),
        correct.len(),
        flagged_share * 100.0
    );
    println!(
        "errors found: {} of {} ({:.1} %), missed {missed:?}",
        errors.len() - missed.len(),
        errors.len(),
        found_share * 100.0
    );
    assert!(correct.len() >= 1000 && errors.len() >= 200);
    assert!(flagged_share <= 0.03);
    assert!(found_share >= 0.95);
}

fn autocorrect(service: &LanguageService, word: &str) -> Result<String, Blocked> {
    let candidate = Candidate {
        word,
        enabled: true,
        span: Span::Text,
    };
    service.autocorrect(
        candidate,
        &["de-DE".to_owned()],
        &OwnWords::default(),
        &|_, _| false,
    )
}

#[test]
fn the_autocorrect_changes_no_correct_word_and_corrects_no_error_wrongly() {
    let service = service();
    for word in lines("de-richtig.txt") {
        assert!(autocorrect(&service, &word).is_err(), "{word} was changed");
    }
    let mut corrected = Vec::new();
    let mut wrong = Vec::new();
    for (error, right) in errors() {
        if let Ok(correction) = autocorrect(&service, &error) {
            if correction == right {
                corrected.push(error);
            } else {
                wrong.push(format!("{error} -> {correction} (right: {right})"));
            }
        }
    }
    println!("corrected automatically: {} {corrected:?}", corrected.len());
    assert!(wrong.is_empty(), "corrected wrongly: {wrong:?}");
    assert!(!corrected.is_empty());
}

#[test]
fn the_shipped_dictionaries_match_their_checksums() {
    let folder = root().join("dictionaries");
    let sums = fs::read_to_string(folder.join("SHA256SUMS")).unwrap();
    let mut checked = 0;
    for line in sums.lines() {
        let (sum, path) = line.split_once("  ").unwrap();
        let data = fs::read(folder.join(path.trim_start_matches("./"))).unwrap();
        let actual: String = Sha256::digest(&data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(actual, sum, "{path}");
        checked += 1;
    }
    assert!(checked >= 15);
}

#[test]
fn every_shipped_dictionary_loads() {
    let store = DictionaryStore::new(
        Some(root().join("dictionaries")),
        root().join("target/no-added-dictionaries"),
    );
    for language in store.languages() {
        let start = std::time::Instant::now();
        let dictionary = store.get(&language.tag).unwrap();
        println!("{} loaded in {:?}", language.tag, start.elapsed());
        assert!(dictionary.check(if language.tag.starts_with("de") {
            "Haus"
        } else {
            "house"
        }));
    }
}
