//! What the language check keeps in the library (section 14, ADR 0025):
//! own dictionaries with their words and targets, rejected autocorrections
//! and the language profile of each document.
//!
//! An own dictionary applies to every document (`forAll`, "Gesamte
//! Anwendung" within the library), to chosen documents, to chosen subnets
//! (and their child subnets), or to several of these at once. For a
//! document the union counts: a word is correct if one of them has it.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::dictionaries::is_tag;
use super::engine::OwnWords;
use crate::clock::Generators;
use crate::database::Database;
use crate::database::language as rows;

/// Longest name of an own dictionary.
pub const MAX_NAME_CHARS: usize = 100;
/// Longest word.
pub const MAX_WORD_CHARS: usize = 100;
/// Most targets one dictionary takes per kind.
pub const MAX_TARGETS: usize = 10_000;
/// Most words one import takes.
pub const MAX_IMPORT_WORDS: usize = 200_000;
/// Most languages in a profile.
pub const MAX_PROFILE_LANGUAGES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WordCategory {
    Word,
    /// A technical term.
    Term,
    /// A proper name.
    Name,
    Abbreviation,
}

impl WordCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Term => "term",
            Self::Name => "name",
            Self::Abbreviation => "abbreviation",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "term" => Self::Term,
            "name" => Self::Name,
            "abbreviation" => Self::Abbreviation,
            _ => Self::Word,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OwnDictionary {
    pub id: String,
    pub name: String,
    pub for_all: bool,
    pub words: u32,
    pub documents: Vec<String>,
    pub subnets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OwnWord {
    pub word: String,
    pub category: WordCategory,
}

/// The result of importing a word list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WordImport {
    pub added: u32,
    pub known: u32,
    /// Lines that are no single word (spaces inside, too long).
    pub skipped: u32,
}

/// The language profile of a document: its languages, the first is the
/// main one (empty: the default of the app), and the autocorrect switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentLanguage {
    pub languages: Vec<String>,
    pub autocorrect: bool,
}

impl Default for DocumentLanguage {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            autocorrect: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OwnError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("no such dictionary")]
    NotFound,
    #[error("a dictionary has this name")]
    NameTaken,
    #[error("bad name")]
    BadName,
    #[error("not a single word")]
    BadWord,
    #[error("no such word")]
    WordNotFound,
    #[error("a target does not exist")]
    BadTarget,
    #[error("bad language")]
    BadLanguage,
    #[error("too many entries")]
    TooMany,
}

/// A name as stored, and the key for its uniqueness.
fn checked_name(name: &str) -> Result<(String, String), OwnError> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0 || count > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(OwnError::BadName);
    }
    Ok((name.to_owned(), name.to_lowercase()))
}

/// A word as stored: trimmed, apostrophes as in the dictionaries, no white
/// space or control characters inside.
pub fn checked_word(word: &str) -> Result<String, OwnError> {
    let word = super::engine::normalize(word);
    let count = word.chars().count();
    if count == 0
        || count > MAX_WORD_CHARS
        || word.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(OwnError::BadWord);
    }
    Ok(word)
}

/// The words of an exchange file (UTF-8, one entry per line). A Hunspell
/// word list works too: a first line with only a count is skipped, flags
/// after `/` are dropped.
pub fn parse_word_list(text: &str) -> (Vec<String>, u32) {
    let mut words = Vec::new();
    let mut skipped = 0u32;
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || (index == 0 && line.bytes().all(|b| b.is_ascii_digit()))
        {
            continue;
        }
        let entry = line.split_once('/').map_or(line, |(word, _)| word);
        match checked_word(entry) {
            Ok(word) => words.push(word),
            Err(_) => skipped += 1,
        }
    }
    (words, skipped)
}

pub struct OwnData {
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl OwnData {
    pub fn new(
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            database,
            generators,
            write_lock,
        }
    }

    fn now(&self) -> String {
        self.generators.now().to_iso()
    }

    pub async fn dictionaries(&self) -> Result<Vec<OwnDictionary>, OwnError> {
        Ok(rows::dictionaries(self.database.reader())
            .await?
            .into_iter()
            .map(|row| OwnDictionary {
                id: row.id,
                name: row.name,
                for_all: row.for_all,
                words: u32::try_from(row.words).unwrap_or(u32::MAX),
                documents: row.documents,
                subnets: row.subnets,
            })
            .collect())
    }

    async fn find(&self, id: &str) -> Result<OwnDictionary, OwnError> {
        self.dictionaries()
            .await?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(OwnError::NotFound)
    }

    pub async fn create(&self, name: &str, for_all: bool) -> Result<OwnDictionary, OwnError> {
        let (name, key) = checked_name(name)?;
        let id = self.generators.new_id().hyphenated().to_string();
        {
            let _guard = self.write_lock.lock().await;
            let mut tx = self.database.writer().begin().await?;
            if rows::dictionary_named(&mut tx, &key).await?.is_some() {
                return Err(OwnError::NameTaken);
            }
            rows::insert_dictionary(&mut tx, &id, &name, &key, for_all, &self.now()).await?;
            tx.commit().await?;
        }
        self.find(&id).await
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<OwnDictionary, OwnError> {
        let (name, key) = checked_name(name)?;
        {
            let _guard = self.write_lock.lock().await;
            let mut tx = self.database.writer().begin().await?;
            if rows::dictionary_named(&mut tx, &key)
                .await?
                .is_some_and(|other| other != id)
            {
                return Err(OwnError::NameTaken);
            }
            if !rows::rename_dictionary(&mut tx, id, &name, &key, &self.now()).await? {
                return Err(OwnError::NotFound);
            }
            tx.commit().await?;
        }
        self.find(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), OwnError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::delete_dictionary(&mut tx, id).await? {
            return Err(OwnError::NotFound);
        }
        tx.commit().await?;
        Ok(())
    }

    /// Replaces where a dictionary applies.
    pub async fn assign(
        &self,
        id: &str,
        for_all: bool,
        documents: &[String],
        subnets: &[String],
    ) -> Result<OwnDictionary, OwnError> {
        let mut documents = documents.to_vec();
        let mut subnets = subnets.to_vec();
        documents.sort();
        documents.dedup();
        subnets.sort();
        subnets.dedup();
        if documents.len() > MAX_TARGETS || subnets.len() > MAX_TARGETS {
            return Err(OwnError::TooMany);
        }
        {
            let _guard = self.write_lock.lock().await;
            let mut tx = self.database.writer().begin().await?;
            if !rows::dictionary_exists(&mut tx, id).await? {
                return Err(OwnError::NotFound);
            }
            if rows::count_documents(&mut tx, &documents).await? != documents.len()
                || rows::count_subnets(&mut tx, &subnets).await? != subnets.len()
            {
                return Err(OwnError::BadTarget);
            }
            rows::set_targets(&mut tx, id, for_all, &documents, &subnets, &self.now()).await?;
            tx.commit().await?;
        }
        self.find(id).await
    }

    pub async fn words(&self, id: &str) -> Result<Vec<OwnWord>, OwnError> {
        let words = rows::words(self.database.reader(), id).await?;
        if words.is_empty() {
            // Distinguish an empty dictionary from a missing one.
            self.find(id).await?;
        }
        Ok(words
            .into_iter()
            .map(|row| OwnWord {
                word: row.word,
                category: WordCategory::parse(&row.category),
            })
            .collect())
    }

    /// Adds a word or changes its category; returns whether it was new.
    pub async fn add_word(
        &self,
        id: &str,
        word: &str,
        category: WordCategory,
    ) -> Result<bool, OwnError> {
        let word = checked_word(word)?;
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::dictionary_exists(&mut tx, id).await? {
            return Err(OwnError::NotFound);
        }
        let added = rows::upsert_word(&mut tx, id, &word, category.as_str(), &self.now()).await?;
        tx.commit().await?;
        Ok(added)
    }

    /// Changes a word (spelling and category) in one step.
    pub async fn update_word(
        &self,
        id: &str,
        old: &str,
        word: &str,
        category: WordCategory,
    ) -> Result<(), OwnError> {
        let old = checked_word(old)?;
        let word = checked_word(word)?;
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::delete_word(&mut tx, id, &old).await? {
            return Err(if rows::dictionary_exists(&mut tx, id).await? {
                OwnError::WordNotFound
            } else {
                OwnError::NotFound
            });
        }
        rows::upsert_word(&mut tx, id, &word, category.as_str(), &self.now()).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_word(&self, id: &str, word: &str) -> Result<(), OwnError> {
        let word = checked_word(word)?;
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::delete_word(&mut tx, id, &word).await? {
            return Err(if rows::dictionary_exists(&mut tx, id).await? {
                OwnError::WordNotFound
            } else {
                OwnError::NotFound
            });
        }
        tx.commit().await?;
        Ok(())
    }

    /// "Add to dictionary" from the editor: into the first dictionary for
    /// all documents, made with `default_name` if there is none.
    pub async fn add_to_default(
        &self,
        word: &str,
        default_name: &str,
    ) -> Result<OwnDictionary, OwnError> {
        let word = checked_word(word)?;
        let id = {
            let _guard = self.write_lock.lock().await;
            let mut tx = self.database.writer().begin().await?;
            let id = match rows::first_for_all(&mut tx).await? {
                Some(id) => id,
                None => {
                    let (name, key) = checked_name(default_name)?;
                    // A dictionary of that name that is not for all: a
                    // free name next to it.
                    let (name, key) = if rows::dictionary_named(&mut tx, &key).await?.is_some() {
                        (format!("{name} (2)"), format!("{key} (2)"))
                    } else {
                        (name, key)
                    };
                    let id = self.generators.new_id().hyphenated().to_string();
                    rows::insert_dictionary(&mut tx, &id, &name, &key, true, &self.now()).await?;
                    id
                }
            };
            rows::upsert_word(
                &mut tx,
                &id,
                &word,
                WordCategory::Word.as_str(),
                &self.now(),
            )
            .await?;
            tx.commit().await?;
            id
        };
        self.find(&id).await
    }

    /// Adds the words of an exchange file; known words stay as they are.
    pub async fn import_words(&self, id: &str, text: &str) -> Result<WordImport, OwnError> {
        let (words, skipped) = parse_word_list(text);
        if words.len() > MAX_IMPORT_WORDS {
            return Err(OwnError::TooMany);
        }
        let known: std::collections::HashSet<String> = self
            .words(id)
            .await?
            .into_iter()
            .map(|entry| entry.word)
            .collect();
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::dictionary_exists(&mut tx, id).await? {
            return Err(OwnError::NotFound);
        }
        let now = self.now();
        let mut added = 0u32;
        let mut already = 0u32;
        let mut seen = std::collections::HashSet::new();
        for word in words {
            if known.contains(&word) || !seen.insert(word.clone()) {
                already += 1;
                continue;
            }
            rows::upsert_word(&mut tx, id, &word, WordCategory::Word.as_str(), &now).await?;
            added += 1;
        }
        tx.commit().await?;
        Ok(WordImport {
            added,
            known: already,
            skipped,
        })
    }

    /// The exchange file of a dictionary: UTF-8, one word per line.
    pub async fn export_words(&self, id: &str) -> Result<String, OwnError> {
        let words = self.words(id).await?;
        let mut text: String = words.into_iter().map(|entry| entry.word + "\n").collect();
        if text.is_empty() {
            text.push('\n');
        }
        Ok(text)
    }

    /// The own words that apply to a document (none: an external file).
    pub async fn own_words(&self, document: Option<&str>) -> Result<OwnWords, OwnError> {
        Ok(OwnWords::new(
            rows::words_for(self.database.reader(), document).await?,
        ))
    }

    pub async fn is_rejected(&self, word: &str, correction: &str) -> Result<bool, OwnError> {
        Ok(rows::is_rejected(self.database.reader(), word, correction).await?)
    }

    /// The corrections of a word the user took back (condition 7).
    pub async fn rejected_for(
        &self,
        word: &str,
    ) -> Result<std::collections::HashSet<String>, OwnError> {
        Ok(rows::rejected_corrections(self.database.reader(), word)
            .await?
            .into_iter()
            .collect())
    }

    pub async fn reject(&self, word: &str, correction: &str) -> Result<(), OwnError> {
        let word = checked_word(word)?;
        let correction = checked_word(correction)?;
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        rows::reject(&mut tx, &word, &correction, &self.now()).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn rejections(&self) -> Result<Vec<(String, String)>, OwnError> {
        Ok(rows::rejections(self.database.reader()).await?)
    }

    pub async fn forget_rejection(&self, word: &str, correction: &str) -> Result<(), OwnError> {
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::forget_rejection(&mut tx, word, correction).await? {
            return Err(OwnError::WordNotFound);
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn profile(&self, document: &str) -> Result<DocumentLanguage, OwnError> {
        Ok(
            match rows::profile(self.database.reader(), document).await? {
                Some(row) => DocumentLanguage {
                    languages: serde_json::from_str(&row.languages).unwrap_or_default(),
                    autocorrect: row.autocorrect,
                },
                None => DocumentLanguage::default(),
            },
        )
    }

    pub async fn set_profile(
        &self,
        document: &str,
        profile: &DocumentLanguage,
    ) -> Result<DocumentLanguage, OwnError> {
        let mut languages: Vec<String> = Vec::new();
        for language in &profile.languages {
            if !is_tag(language) {
                return Err(OwnError::BadLanguage);
            }
            if !languages.contains(language) {
                languages.push(language.clone());
            }
        }
        if languages.len() > MAX_PROFILE_LANGUAGES {
            return Err(OwnError::TooMany);
        }
        let stored = DocumentLanguage {
            languages,
            autocorrect: profile.autocorrect,
        };
        let json = serde_json::to_string(&stored.languages).unwrap_or_else(|_| "[]".to_owned());
        let _guard = self.write_lock.lock().await;
        let mut tx = self.database.writer().begin().await?;
        if !rows::document_exists(&mut tx, document).await? {
            return Err(OwnError::BadTarget);
        }
        rows::set_profile(&mut tx, document, &json, stored.autocorrect, &self.now()).await?;
        tx.commit().await?;
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_single_and_trimmed() {
        assert_eq!(checked_word("  Tauri ").unwrap(), "Tauri");
        assert_eq!(checked_word("geht’s").unwrap(), "geht's");
        for bad in ["", "zwei Wörter", "Tab\there", &"x".repeat(101)] {
            assert!(checked_word(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn word_lists_take_plain_and_hunspell_files() {
        let (words, skipped) =
            parse_word_list("\u{feff}3\nTauri\nRust/AB\n# Kommentar\n\nzwei Wörter\nSQLx\n");
        assert_eq!(words, ["Tauri", "Rust", "SQLx"]);
        assert_eq!(skipped, 1);
        // A number on a later line is a word like any other.
        assert_eq!(parse_word_list("Eins\n42\n").0, ["Eins", "42"]);
    }
}
