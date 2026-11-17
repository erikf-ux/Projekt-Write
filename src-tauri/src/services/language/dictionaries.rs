//! Hunspell dictionaries (section 14, ADR 0023, ADR 0024).
//!
//! The app ships `de-DE`, `de-AT`, `de-CH`, `en-US` and `en-GB` as data
//! packages next to the program (`dictionaries/<tag>/<tag>.aff|.dic` with
//! their license texts), never compiled in. The user may add further
//! language dictionaries as `.aff` and `.dic`; they are kept in the app
//! folder (`dictionaries/<tag>/`) and win over a bundled one of the same
//! tag. A dictionary is loaded on first use and kept.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use spellbook::Dictionary;
use ts_rs::TS;

use super::engine::EngineError;
use crate::filesystem::atomic::write_atomic;

/// Largest affix and word files an import takes.
pub const MAX_AFF_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_DIC_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DictionarySource {
    /// Shipped with the app.
    Bundled,
    /// Added by the user.
    Added,
}

/// A language dictionary the check can use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LanguageDictionary {
    /// BCP 47 tag, `de-DE`.
    pub tag: String,
    pub source: DictionarySource,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("not a language tag")]
    BadTag,
    #[error("file too large")]
    TooLarge,
    #[error("unsupported encoding {0}")]
    Encoding(String),
    #[error("not a Hunspell dictionary: {0}")]
    Invalid(String),
    #[error("a bundled dictionary cannot be removed")]
    Bundled,
    #[error("no such dictionary")]
    NotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `de`, `de-DE`, `en-GB`, `sr-Latn-RS`: letters and digits, parts joined
/// by hyphens. Used as a folder name, so nothing else is allowed.
pub fn is_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    (2..=3).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_lowercase())
        && parts.all(|part| {
            (2..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// The base language of a tag: `de` for `de-CH`.
pub fn base_language(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

pub struct DictionaryStore {
    bundled: Option<PathBuf>,
    added: PathBuf,
    loaded: Mutex<HashMap<String, Arc<Dictionary>>>,
}

impl DictionaryStore {
    /// `bundled`: the folder of the shipped dictionaries (none when it
    /// cannot be found); `added`: where imported ones are kept.
    pub fn new(bundled: Option<PathBuf>, added: PathBuf) -> Self {
        Self {
            bundled,
            added,
            loaded: Mutex::new(HashMap::new()),
        }
    }

    fn files(folder: &Path, tag: &str) -> Option<(PathBuf, PathBuf)> {
        let base = folder.join(tag);
        let aff = base.join(format!("{tag}.aff"));
        let dic = base.join(format!("{tag}.dic"));
        (aff.is_file() && dic.is_file()).then_some((aff, dic))
    }

    fn tags_in(folder: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(folder) else {
            return Vec::new();
        };
        let mut tags: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|tag| is_tag(tag) && Self::files(folder, tag).is_some())
            .collect();
        tags.sort();
        tags
    }

    /// All language dictionaries, sorted by tag.
    pub fn languages(&self) -> Vec<LanguageDictionary> {
        let mut all: Vec<LanguageDictionary> = Self::tags_in(&self.added)
            .into_iter()
            .map(|tag| LanguageDictionary {
                tag,
                source: DictionarySource::Added,
            })
            .collect();
        if let Some(bundled) = &self.bundled {
            for tag in Self::tags_in(bundled) {
                if !all.iter().any(|known| known.tag == tag) {
                    all.push(LanguageDictionary {
                        tag,
                        source: DictionarySource::Bundled,
                    });
                }
            }
        }
        all.sort_by(|a, b| a.tag.cmp(&b.tag));
        all
    }

    /// The dictionary tag for a language: the tag itself if there is one;
    /// for a bare language or an unknown region the first of the same
    /// language, `de-DE` and `en-US` first.
    pub fn resolve(&self, language: &str) -> Option<String> {
        let languages = self.languages();
        if languages.iter().any(|known| known.tag == language) {
            return Some(language.to_owned());
        }
        let base = base_language(language);
        let preferred = match base {
            "de" => Some("de-DE"),
            "en" => Some("en-US"),
            _ => None,
        };
        if let Some(preferred) = preferred
            && languages.iter().any(|known| known.tag == preferred)
        {
            return Some(preferred.to_owned());
        }
        languages
            .into_iter()
            .find(|known| base_language(&known.tag) == base)
            .map(|known| known.tag)
    }

    fn location(&self, tag: &str) -> Option<(PathBuf, PathBuf)> {
        Self::files(&self.added, tag).or_else(|| {
            self.bundled
                .as_deref()
                .and_then(|bundled| Self::files(bundled, tag))
        })
    }

    /// The loaded dictionary of a tag (see [`Self::resolve`]); loads it
    /// on first use, which takes a moment: never call it on the input path.
    pub fn get(&self, tag: &str) -> Result<Arc<Dictionary>, EngineError> {
        if let Some(found) = self.cached(tag) {
            return Ok(found);
        }
        let (aff, dic) = self
            .location(tag)
            .ok_or_else(|| EngineError::NoDictionary(tag.to_owned()))?;
        let read = |path: &Path| {
            fs::read_to_string(path)
                .map_err(|error| EngineError::BadDictionary(tag.to_owned(), error.to_string()))
        };
        let dictionary = Dictionary::new(&read(&aff)?, &read(&dic)?)
            .map_err(|error| EngineError::BadDictionary(tag.to_owned(), error.to_string()))?;
        let dictionary = Arc::new(dictionary);
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(Arc::clone(
            loaded.entry(tag.to_owned()).or_insert(dictionary),
        ))
    }

    fn cached(&self, tag: &str) -> Option<Arc<Dictionary>> {
        let loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loaded.get(tag).cloned()
    }

    fn forget(&self, tag: &str) {
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loaded.remove(tag);
    }

    /// Adds a language dictionary from a Hunspell pair. Both files are read
    /// with a size limit, converted to UTF-8 and parsed before anything is
    /// written; an existing added dictionary of the tag is replaced.
    pub fn import(
        &self,
        tag: &str,
        aff: &Path,
        dic: &Path,
    ) -> Result<LanguageDictionary, ImportError> {
        if !is_tag(tag) {
            return Err(ImportError::BadTag);
        }
        let aff_bytes = read_limited(aff, MAX_AFF_BYTES)?;
        let dic_bytes = read_limited(dic, MAX_DIC_BYTES)?;
        let encoding = declared_encoding(&aff_bytes);
        let aff_text = to_utf8(&aff_bytes, &encoding)?;
        let dic_text = to_utf8(&dic_bytes, &encoding)?;
        let aff_text = declare_utf8(&aff_text);
        Dictionary::new(&aff_text, &dic_text)
            .map_err(|error| ImportError::Invalid(error.to_string()))?;
        let folder = self.added.join(tag);
        fs::create_dir_all(&folder)?;
        write_atomic(&folder.join(format!("{tag}.aff")), aff_text.as_bytes())?;
        write_atomic(&folder.join(format!("{tag}.dic")), dic_text.as_bytes())?;
        self.forget(tag);
        Ok(LanguageDictionary {
            tag: tag.to_owned(),
            source: DictionarySource::Added,
        })
    }

    /// Removes an added language dictionary; a bundled one stays.
    pub fn remove(&self, tag: &str) -> Result<(), ImportError> {
        if !is_tag(tag) {
            return Err(ImportError::BadTag);
        }
        let folder = self.added.join(tag);
        if Self::files(&self.added, tag).is_none() {
            return if self.location(tag).is_some() {
                Err(ImportError::Bundled)
            } else {
                Err(ImportError::NotFound)
            };
        }
        fs::remove_dir_all(folder)?;
        self.forget(tag);
        Ok(())
    }
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ImportError> {
    let size = fs::metadata(path)?.len();
    if size > limit {
        return Err(ImportError::TooLarge);
    }
    Ok(fs::read(path)?)
}

/// The `SET` line of an affix file, upper case; UTF-8 when there is none.
fn declared_encoding(aff: &[u8]) -> String {
    aff.split(|&b| b == b'\n')
        .map(|line| String::from_utf8_lossy(line).trim().to_owned())
        .find_map(|line| {
            line.strip_prefix("SET ")
                .map(|rest| rest.trim().to_uppercase())
        })
        .unwrap_or_else(|| "UTF-8".to_owned())
}

fn to_utf8(bytes: &[u8], encoding: &str) -> Result<String, ImportError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match encoding {
        "UTF-8" | "UTF8" => String::from_utf8(bytes.to_vec())
            .map_err(|_| ImportError::Encoding("UTF-8 (invalid)".to_owned())),
        "ISO8859-1" | "ISO-8859-1" => Ok(bytes.iter().map(|&b| char::from(b)).collect()),
        "ISO8859-15" | "ISO-8859-15" => Ok(bytes.iter().map(|&b| latin9(b)).collect()),
        other => Err(ImportError::Encoding(other.to_owned())),
    }
}

/// ISO 8859-15 differs from ISO 8859-1 in eight places.
fn latin9(byte: u8) -> char {
    match byte {
        0xA4 => '€',
        0xA6 => 'Š',
        0xA8 => 'š',
        0xB4 => 'Ž',
        0xB8 => 'ž',
        0xBC => 'Œ',
        0xBD => 'œ',
        0xBE => 'Ÿ',
        other => char::from(other),
    }
}

/// The affix file as stored: `SET UTF-8`, everything else unchanged.
fn declare_utf8(aff: &str) -> String {
    let mut found = false;
    let mut lines: Vec<String> = aff
        .lines()
        .map(|line| {
            if !found && line.trim_start().starts_with("SET ") {
                found = true;
                "SET UTF-8".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !found {
        lines.insert(0, "SET UTF-8".to_owned());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwz\n";

    fn bundled() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dictionaries")
    }

    #[test]
    fn tags_are_checked_like_folder_names() {
        for tag in ["de", "de-DE", "en-GB", "sr-Latn-RS", "gsw-CH"] {
            assert!(is_tag(tag), "{tag}");
        }
        for tag in ["", "D", "de_DE", "de-", "../de", "de/DE", "DE-de", "de-D"] {
            assert!(!is_tag(tag), "{tag}");
        }
        assert_eq!(base_language("de-CH"), "de");
    }

    #[test]
    fn the_five_bundled_languages_are_found_and_resolved() {
        let added = tempfile::tempdir().unwrap();
        let store = DictionaryStore::new(Some(bundled()), added.path().to_owned());
        let tags: Vec<String> = store.languages().into_iter().map(|l| l.tag).collect();
        assert_eq!(tags, ["de-AT", "de-CH", "de-DE", "en-GB", "en-US"]);
        assert_eq!(store.resolve("de").as_deref(), Some("de-DE"));
        assert_eq!(store.resolve("de-LU").as_deref(), Some("de-DE"));
        assert_eq!(store.resolve("en").as_deref(), Some("en-US"));
        assert_eq!(store.resolve("de-CH").as_deref(), Some("de-CH"));
        assert_eq!(store.resolve("fr-FR"), None);
    }

    #[test]
    fn an_added_dictionary_is_imported_in_utf_8_and_removed_again() {
        let added = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let aff = source.path().join("x.aff");
        let dic = source.path().join("x.dic");
        fs::write(&aff, b"SET ISO8859-1\nTRY e\n").unwrap();
        fs::write(&dic, b"2\nStra\xDFe\nK\xE4se\n").unwrap();
        let store = DictionaryStore::new(None, added.path().to_owned());
        store.import("xx-YY", &aff, &dic).unwrap();
        assert_eq!(store.resolve("xx").as_deref(), Some("xx-YY"));
        let dictionary = store.get("xx-YY").unwrap();
        assert!(dictionary.check("Straße"));
        assert!(dictionary.check("Käse"));
        assert!(!dictionary.check("Strasse"));
        let stored = fs::read_to_string(added.path().join("xx-YY/xx-YY.aff")).unwrap();
        assert!(stored.starts_with("SET UTF-8\n"));
        store.remove("xx-YY").unwrap();
        assert!(store.languages().is_empty());
        assert!(matches!(
            store.get("xx-YY"),
            Err(EngineError::NoDictionary(_))
        ));
    }

    #[test]
    fn imports_refuse_bad_tags_encodings_and_bundled_removals() {
        let added = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let aff = source.path().join("x.aff");
        let dic = source.path().join("x.dic");
        fs::write(&aff, "SET KOI8-R\n").unwrap();
        fs::write(&dic, "1\nслово\n").unwrap();
        let store = DictionaryStore::new(Some(bundled()), added.path().to_owned());
        assert!(matches!(
            store.import("../x", &aff, &dic),
            Err(ImportError::BadTag)
        ));
        assert!(matches!(
            store.import("ru-RU", &aff, &dic),
            Err(ImportError::Encoding(_))
        ));
        fs::write(&aff, AFF).unwrap();
        fs::write(&dic, "not a count\nword\n").unwrap();
        // spellbook takes a missing count; an empty word file is still a
        // dictionary. A file that is not text is refused.
        fs::write(&dic, [0xFF, 0xFE, 0x00]).unwrap();
        assert!(matches!(
            store.import("ru-RU", &aff, &dic),
            Err(ImportError::Encoding(_))
        ));
        assert!(matches!(store.remove("de-DE"), Err(ImportError::Bundled)));
        assert!(matches!(store.remove("fr-FR"), Err(ImportError::NotFound)));
    }
}
