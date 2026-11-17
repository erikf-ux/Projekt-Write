//! The language check (Phase 12, section 14, ADR 0025): checking texts,
//! suggestions, the autocorrect, own dictionaries, language profiles and
//! language dictionaries.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::{CommandError, ErrorCode};
use crate::filesystem::atomic::{read_limited, write_atomic};
use crate::services::dialog::{FileDialog, FileFilter, FilePickRequest, PickRequest};
use crate::services::language::autocorrect::{Blocked, Candidate, Span};
use crate::services::language::dictionaries::{ImportError, LanguageDictionary, is_tag};
use crate::services::language::engine::{LanguageEngine, OwnWords};
use crate::services::language::engines::{self, AddOns};
use crate::services::language::languagetool::{LanguageToolStatus, SetupError, ToolState};
use crate::services::language::own::{
    DocumentLanguage, MAX_NAME_CHARS, MAX_WORD_CHARS, OwnData, OwnDictionary, OwnError, OwnWord,
    WordCategory, WordImport,
};
use crate::services::language::{CheckParts, CheckedText, detect};
use crate::services::locale::Locale;
use crate::services::registry::RegistryEntry;
use crate::services::settings::ui::LanguageCheckSettings;
use crate::state::AppState;

/// Most texts one check request takes.
const MAX_TEXTS: usize = 500;
/// Longest text (characters) and all texts of a request together.
const MAX_TEXT_CHARS: usize = 100_000;
const MAX_REQUEST_CHARS: usize = 2_000_000;
/// Largest word list a dictionary import reads.
const MAX_WORD_LIST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

fn own_error(error: OwnError) -> CommandError {
    match error {
        OwnError::Database(error) => {
            log::error!("language database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
        OwnError::NotFound => CommandError::new(ErrorCode::DictionaryNotFound),
        OwnError::NameTaken => CommandError::new(ErrorCode::DictionaryNameTaken),
        OwnError::WordNotFound => CommandError::new(ErrorCode::DictionaryWordNotFound),
        OwnError::BadName => CommandError::invalid_input("name", "1 to 100 characters"),
        OwnError::BadWord => CommandError::invalid_input("word", "a single word"),
        OwnError::BadTarget => CommandError::invalid_input("target", "does not exist"),
        OwnError::BadLanguage => CommandError::invalid_input("languages", "not a language tag"),
        OwnError::TooMany => CommandError::invalid_input("entries", "too many"),
    }
}

fn is_uuid(id: &str) -> bool {
    Uuid::try_parse(id)
        .ok()
        .is_some_and(|uuid| uuid.hyphenated().to_string() == id)
}

fn check_id(field: &str, id: &str) -> Result<(), CommandError> {
    if is_uuid(id) {
        Ok(())
    } else {
        Err(CommandError::invalid_input(field, "not an id"))
    }
}

fn check_optional_id(field: &str, id: Option<&String>) -> Result<(), CommandError> {
    id.map_or(Ok(()), |id| check_id(field, id))
}

fn check_language(field: &str, language: Option<&String>) -> Result<(), CommandError> {
    match language {
        Some(tag) if !is_tag(tag) => Err(CommandError::invalid_input(field, "not a language tag")),
        _ => Ok(()),
    }
}

fn check_word(word: &str) -> Result<(), CommandError> {
    let count = word.trim().chars().count();
    if count == 0 || count > MAX_WORD_CHARS {
        return Err(CommandError::invalid_input("word", "1 to 100 characters"));
    }
    Ok(())
}

fn check_title(title: &str) -> Result<(), CommandError> {
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(CommandError::invalid_input("title", "too long"));
    }
    Ok(())
}

fn settings(state: &AppState) -> LanguageCheckSettings {
    state.ui.state().settings.language_check
}

/// The language of documents without a profile.
fn default_language(state: &AppState) -> String {
    let ui = state.ui.state();
    ui.settings
        .language_check
        .default_language
        .unwrap_or_else(|| match ui.locale {
            Locale::De => "de-DE".to_owned(),
            Locale::En => "en-US".to_owned(),
        })
}

/// What a check needs from the library: the own words and the profile of
/// the document. Without an open library: no own words, no profile.
async fn context(
    state: &AppState,
    document: Option<String>,
) -> Result<(OwnWords, DocumentLanguage), CommandError> {
    let result = state
        .session
        .with_language(|data| async move {
            let own = data
                .own_words(document.as_deref())
                .await
                .map_err(own_error)?;
            let profile = match document.as_deref() {
                Some(id) => data.profile(id).await.map_err(own_error)?,
                None => DocumentLanguage::default(),
            };
            Ok((own, profile))
        })
        .await;
    match result {
        Err(error) if error.code == ErrorCode::LibraryNotOpen => {
            Ok((OwnWords::default(), DocumentLanguage::default()))
        }
        other => other,
    }
}

/// The languages of a document: its profile, or the default of the app.
fn profile_languages(state: &AppState, profile: &DocumentLanguage) -> Vec<String> {
    if profile.languages.is_empty() {
        vec![default_language(state)]
    } else {
        profile.languages.clone()
    }
}

/// The dictionaries the check can use.
#[tauri::command]
pub async fn language_dictionaries(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<Vec<LanguageDictionary>, CommandError> {
    let language = state.language.clone();
    tauri::async_runtime::spawn_blocking(move || language.languages())
        .await
        .map_err(|_| CommandError::new(ErrorCode::Internal))
}

/// One text of a check: a paragraph, or a part of it marked with a
/// language. The editor masks code and links with U+FFFC.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CheckTextRequest {
    pub text: String,
    /// A language the text is marked with; none: the document's.
    #[serde(default)]
    #[ts(optional)]
    pub language: Option<String>,
    /// The language of the paragraph before, for a paragraph too short to
    /// be detected.
    #[serde(default)]
    #[ts(optional)]
    pub inherit: Option<String>,
}

/// Request of `language_check`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageCheckRequest {
    /// The library document; none for an external file.
    #[serde(default)]
    #[ts(optional)]
    pub document_id: Option<String>,
    pub texts: Vec<CheckTextRequest>,
}

impl Validate for LanguageCheckRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_optional_id("documentId", self.document_id.as_ref())?;
        if self.texts.len() > MAX_TEXTS {
            return Err(CommandError::invalid_input("texts", "too many"));
        }
        let mut total = 0;
        for text in &self.texts {
            let count = text.text.chars().count();
            if count > MAX_TEXT_CHARS {
                return Err(CommandError::invalid_input("texts", "a text is too long"));
            }
            total += count;
            check_language("language", text.language.as_ref())?;
            check_language("inherit", text.inherit.as_ref())?;
        }
        if total > MAX_REQUEST_CHARS {
            return Err(CommandError::invalid_input("texts", "too long together"));
        }
        Ok(())
    }
}

/// The add-on engines of the registry that run now, with their instance:
/// LanguageTool while the check is on and the add-on runs (ADR 0026).
fn running_add_ons(
    state: &AppState,
    enabled: bool,
) -> Vec<(&'static str, Arc<dyn LanguageEngine>)> {
    let mut running: Vec<(&'static str, Arc<dyn LanguageEngine>)> = Vec::new();
    for entry in state.extensions.language_engines.iter() {
        if !entry.add_on() || !enabled {
            continue;
        }
        if entry.id() == engines::LANGUAGETOOL
            && state.language_tool.status().state == ToolState::Running
        {
            running.push((entry.id(), state.language_tool.clone()));
        }
    }
    running
}

/// Checks texts of a document, off the input path (blocking pool).
#[tauri::command]
pub async fn language_check(
    state: State<'_, AppState>,
    request: Input<LanguageCheckRequest>,
) -> Result<Vec<CheckedText>, CommandError> {
    let request = request.into_inner();
    let settings = settings(&state);
    let (own, profile) = context(&state, request.document_id.clone()).await?;
    let languages = profile_languages(&state, &profile);
    let parts = CheckParts {
        spelling: settings.enabled,
        rules: settings.enabled && settings.rules,
    };
    let service = state.language.clone();
    let default = default_language(&state);
    let detect = settings.detect;
    let running = running_add_ons(&state, settings.enabled);
    tauri::async_runtime::spawn_blocking(move || {
        let add_ons = running
            .iter()
            .fold(AddOns::none(), |add_ons, (id, engine)| {
                add_ons.with(id, &**engine)
            });
        request
            .texts
            .iter()
            .map(|text| {
                // Section 14: a marked part, the detection, the paragraph
                // before (short paragraphs), the profile, the default.
                let language = text.language.clone().unwrap_or_else(|| {
                    match detect.then(|| detect::detect(&text.text)).flatten() {
                        Some(found) => detect::choose(found, &languages, &default, service.store()),
                        None => text.inherit.clone().unwrap_or_else(|| languages[0].clone()),
                    }
                });
                service.check(&text.text, &language, &languages, &own, parts, &add_ons)
            })
            .collect()
    })
    .await
    .map_err(|_| CommandError::new(ErrorCode::Internal))
}

/// Request of `language_suggest`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageSuggestRequest {
    #[serde(default)]
    #[ts(optional)]
    pub document_id: Option<String>,
    pub word: String,
    #[serde(default)]
    #[ts(optional)]
    pub language: Option<String>,
}

impl Validate for LanguageSuggestRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_optional_id("documentId", self.document_id.as_ref())?;
        check_word(&self.word)?;
        check_language("language", self.language.as_ref())
    }
}

/// At most five suggestions for a word, in the order of section 14.
#[tauri::command]
pub async fn language_suggest(
    state: State<'_, AppState>,
    request: Input<LanguageSuggestRequest>,
) -> Result<Vec<String>, CommandError> {
    let request = request.into_inner();
    let (own, profile) = context(&state, request.document_id.clone()).await?;
    let language = request
        .language
        .clone()
        .unwrap_or_else(|| profile_languages(&state, &profile)[0].clone());
    let service = state.language.clone();
    tauri::async_runtime::spawn_blocking(move || service.suggest(&request.word, &language, &own))
        .await
        .map_err(|_| CommandError::new(ErrorCode::Internal))?
        .map_err(|_| CommandError::new(ErrorCode::LanguageNoDictionary))
}

/// Request of `language_autocorrect`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AutocorrectRequest {
    #[serde(default)]
    #[ts(optional)]
    pub document_id: Option<String>,
    pub word: String,
    /// The language of the section; none: the document's.
    #[serde(default)]
    #[ts(optional)]
    pub language: Option<String>,
    pub span: Span,
}

impl Validate for AutocorrectRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_optional_id("documentId", self.document_id.as_ref())?;
        check_word(&self.word)?;
        check_language("language", self.language.as_ref())
    }
}

/// The answer of the autocorrect: the correction, or the number of the
/// condition of section 14 that blocks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutocorrectAnswer {
    pub correction: Option<String>,
    pub blocked_by: Option<u8>,
}

/// Decides whether a finished word is corrected automatically.
#[tauri::command]
pub async fn language_autocorrect(
    state: State<'_, AppState>,
    request: Input<AutocorrectRequest>,
) -> Result<AutocorrectAnswer, CommandError> {
    let request = request.into_inner();
    let settings = settings(&state);
    let (own, profile) = context(&state, request.document_id.clone()).await?;
    let enabled = settings.enabled && settings.autocorrect && profile.autocorrect;
    let blocked = |condition: Blocked| AutocorrectAnswer {
        correction: None,
        blocked_by: Some(condition.condition()),
    };
    if !enabled {
        return Ok(blocked(Blocked::Disabled));
    }
    let word = request.word.trim().to_owned();
    let rejected: HashSet<String> = match state
        .session
        .with_language(|data| async move { data.rejected_for(&word).await.map_err(own_error) })
        .await
    {
        Ok(found) => found,
        Err(error) if error.code == ErrorCode::LibraryNotOpen => HashSet::new(),
        Err(error) => return Err(error),
    };
    let mut languages = profile_languages(&state, &profile);
    if let Some(language) = &request.language {
        languages.retain(|other| other != language);
        languages.insert(0, language.clone());
    }
    let service = state.language.clone();
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let candidate = Candidate {
            word: request.word.trim(),
            enabled,
            span: request.span,
        };
        service.autocorrect(candidate, &languages, &own, &|_, correction| {
            rejected.contains(correction)
        })
    })
    .await
    .map_err(|_| CommandError::new(ErrorCode::Internal))?;
    Ok(match answer {
        Ok(correction) => AutocorrectAnswer {
            correction: Some(correction),
            blocked_by: None,
        },
        Err(condition) => blocked(condition),
    })
}

/// Request of `language_reject` and `autocorrect_forget`.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Rejection {
    pub word: String,
    pub correction: String,
}

impl Validate for Rejection {
    fn validate(&self) -> Result<(), CommandError> {
        check_word(&self.word)?;
        check_word(&self.correction)
    }
}

/// Ctrl+Z took a correction back: it is never made again (condition 7).
#[tauri::command]
pub async fn language_reject(
    state: State<'_, AppState>,
    request: Input<Rejection>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.reject(&request.word, &request.correction)
                .await
                .map_err(own_error)
        })
        .await
}

/// The corrections the user took back.
#[tauri::command]
pub async fn autocorrect_rejections(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<Vec<Rejection>, CommandError> {
    state
        .session
        .with_language(|data| async move {
            Ok(data
                .rejections()
                .await
                .map_err(own_error)?
                .into_iter()
                .map(|(word, correction)| Rejection { word, correction })
                .collect())
        })
        .await
}

/// Allows a taken-back correction again.
#[tauri::command]
pub async fn autocorrect_forget(
    state: State<'_, AppState>,
    request: Input<Rejection>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.forget_rejection(&request.word, &request.correction)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request of `language_profile`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DocumentIdRequest {
    pub document_id: String,
}

impl Validate for DocumentIdRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("documentId", &self.document_id)
    }
}

/// The language profile of a document.
#[tauri::command]
pub async fn language_profile(
    state: State<'_, AppState>,
    request: Input<DocumentIdRequest>,
) -> Result<DocumentLanguage, CommandError> {
    let id = request.into_inner().document_id;
    state
        .session
        .with_language(|data| async move { data.profile(&id).await.map_err(own_error) })
        .await
}

/// Request of `language_profile_set`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ProfileRequest {
    pub document_id: String,
    pub profile: DocumentLanguage,
}

impl Validate for ProfileRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("documentId", &self.document_id)?;
        for language in &self.profile.languages {
            check_language("languages", Some(language))?;
        }
        Ok(())
    }
}

/// Sets the languages of a document and its autocorrect switch.
#[tauri::command]
pub async fn language_profile_set(
    state: State<'_, AppState>,
    request: Input<ProfileRequest>,
) -> Result<DocumentLanguage, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.set_profile(&request.document_id, &request.profile)
                .await
                .map_err(own_error)
        })
        .await
}

/// The own dictionaries of the library.
#[tauri::command]
pub async fn dictionaries_list(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<Vec<OwnDictionary>, CommandError> {
    state
        .session
        .with_language(|data| async move { data.dictionaries().await.map_err(own_error) })
        .await
}

/// Request of `dictionary_create`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryCreateRequest {
    pub name: String,
    pub for_all: bool,
}

impl Validate for DictionaryCreateRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.name.trim().is_empty() || self.name.chars().count() > MAX_NAME_CHARS {
            return Err(CommandError::invalid_input("name", "1 to 100 characters"));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn dictionary_create(
    state: State<'_, AppState>,
    request: Input<DictionaryCreateRequest>,
) -> Result<OwnDictionary, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.create(&request.name, request.for_all)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request of `dictionary_rename`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryRenameRequest {
    pub id: String,
    pub name: String,
}

impl Validate for DictionaryRenameRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        if self.name.trim().is_empty() || self.name.chars().count() > MAX_NAME_CHARS {
            return Err(CommandError::invalid_input("name", "1 to 100 characters"));
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn dictionary_rename(
    state: State<'_, AppState>,
    request: Input<DictionaryRenameRequest>,
) -> Result<OwnDictionary, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.rename(&request.id, &request.name)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request with a dictionary id.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryIdRequest {
    pub id: String,
}

impl Validate for DictionaryIdRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)
    }
}

#[tauri::command]
pub async fn dictionary_delete(
    state: State<'_, AppState>,
    request: Input<DictionaryIdRequest>,
) -> Result<(), CommandError> {
    let id = request.into_inner().id;
    state
        .session
        .with_language(|data| async move { data.delete(&id).await.map_err(own_error) })
        .await
}

/// Request of `dictionary_assign`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryAssignRequest {
    pub id: String,
    pub for_all: bool,
    pub documents: Vec<String>,
    pub subnets: Vec<String>,
}

impl Validate for DictionaryAssignRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        for id in &self.documents {
            check_id("documents", id)?;
        }
        for id in &self.subnets {
            check_id("subnets", id)?;
        }
        Ok(())
    }
}

/// Replaces where a dictionary applies: all documents, chosen documents,
/// chosen subnets, or several at once.
#[tauri::command]
pub async fn dictionary_assign(
    state: State<'_, AppState>,
    request: Input<DictionaryAssignRequest>,
) -> Result<OwnDictionary, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.assign(
                &request.id,
                request.for_all,
                &request.documents,
                &request.subnets,
            )
            .await
            .map_err(own_error)
        })
        .await
}

#[tauri::command]
pub async fn dictionary_words(
    state: State<'_, AppState>,
    request: Input<DictionaryIdRequest>,
) -> Result<Vec<OwnWord>, CommandError> {
    let id = request.into_inner().id;
    state
        .session
        .with_language(|data| async move { data.words(&id).await.map_err(own_error) })
        .await
}

/// Request of `dictionary_word_add` and `dictionary_word_remove`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryWordRequest {
    pub id: String,
    pub word: String,
    #[serde(default)]
    #[ts(optional)]
    pub category: Option<WordCategory>,
}

impl Validate for DictionaryWordRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        check_word(&self.word)
    }
}

/// Adds a word (or changes its category); `true` when it was new.
#[tauri::command]
pub async fn dictionary_word_add(
    state: State<'_, AppState>,
    request: Input<DictionaryWordRequest>,
) -> Result<bool, CommandError> {
    let request = request.into_inner();
    let category = request.category.unwrap_or(WordCategory::Word);
    state
        .session
        .with_language(|data| async move {
            data.add_word(&request.id, &request.word, category)
                .await
                .map_err(own_error)
        })
        .await
}

#[tauri::command]
pub async fn dictionary_word_remove(
    state: State<'_, AppState>,
    request: Input<DictionaryWordRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.remove_word(&request.id, &request.word)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request of `dictionary_word_update`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryWordUpdateRequest {
    pub id: String,
    pub old: String,
    pub word: String,
    pub category: WordCategory,
}

impl Validate for DictionaryWordUpdateRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        check_word(&self.old)?;
        check_word(&self.word)
    }
}

#[tauri::command]
pub async fn dictionary_word_update(
    state: State<'_, AppState>,
    request: Input<DictionaryWordUpdateRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.update_word(&request.id, &request.old, &request.word, request.category)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request of `dictionary_add_word`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddWordRequest {
    pub word: String,
    /// Name of the dictionary made when there is none for all documents.
    pub default_name: String,
}

impl Validate for AddWordRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_word(&self.word)?;
        if self.default_name.trim().is_empty() || self.default_name.chars().count() > MAX_NAME_CHARS
        {
            return Err(CommandError::invalid_input(
                "defaultName",
                "1 to 100 characters",
            ));
        }
        Ok(())
    }
}

/// "Add to dictionary" from the editor, one click.
#[tauri::command]
pub async fn dictionary_add_word(
    state: State<'_, AppState>,
    request: Input<AddWordRequest>,
) -> Result<OwnDictionary, CommandError> {
    let request = request.into_inner();
    state
        .session
        .with_language(|data| async move {
            data.add_to_default(&request.word, &request.default_name)
                .await
                .map_err(own_error)
        })
        .await
}

/// Request of the import and export of a word list.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DictionaryFileRequest {
    pub id: String,
    /// Title of the file dialog.
    pub title: String,
}

impl Validate for DictionaryFileRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_id("id", &self.id)?;
        check_title(&self.title)
    }
}

fn word_list_filter() -> FileFilter {
    FileFilter {
        name: "Wortliste".to_owned(),
        extensions: vec!["txt".to_owned(), "dic".to_owned()],
    }
}

/// Imports a word list (UTF-8, one entry per line) into a dictionary; none
/// when the dialog was cancelled.
#[tauri::command]
pub async fn dictionary_import(
    state: State<'_, AppState>,
    request: Input<DictionaryFileRequest>,
) -> Result<Option<WordImport>, CommandError> {
    let request = request.into_inner();
    let data: OwnData = state
        .session
        .with_language(|data| async move { Ok(data) })
        .await?;
    let answer = state.language_files.pick_file(
        FileDialog::Open,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: None,
            filters: vec![word_list_filter()],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(None);
    };
    let bytes = read_limited(&file, MAX_WORD_LIST_BYTES).map_err(|error| {
        log::warn!("word list unreadable: {error}");
        CommandError::new(ErrorCode::FileSystem)
    })?;
    let text =
        String::from_utf8(bytes).map_err(|_| CommandError::invalid_input("file", "not UTF-8"))?;
    data.import_words(&request.id, &text)
        .await
        .map(Some)
        .map_err(own_error)
}

/// Exports the words of a dictionary as a word list; `false` when the
/// dialog was cancelled.
#[tauri::command]
pub async fn dictionary_export(
    state: State<'_, AppState>,
    request: Input<DictionaryFileRequest>,
) -> Result<bool, CommandError> {
    let request = request.into_inner();
    let id = request.id.clone();
    let (name, text) = state
        .session
        .with_language(|data| async move {
            let name = data
                .dictionaries()
                .await
                .map_err(own_error)?
                .into_iter()
                .find(|d| d.id == id)
                .map(|d| d.name)
                .ok_or_else(|| CommandError::new(ErrorCode::DictionaryNotFound))?;
            Ok((name, data.export_words(&id).await.map_err(own_error)?))
        })
        .await?;
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let answer = state.language_files.pick_file(
        FileDialog::Save,
        FilePickRequest {
            title: request.title,
            start_in: None,
            file_name: Some(format!("{}.txt", safe.trim())),
            filters: vec![word_list_filter()],
        },
    );
    let Ok(Some(file)) = answer.await else {
        return Ok(false);
    };
    write_atomic(&file, text.as_bytes()).map_err(|error| {
        log::warn!("word list not written: {error}");
        CommandError::new(ErrorCode::FileSystem)
    })?;
    Ok(true)
}

/// Request of `language_dictionary_import`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageDictionaryImportRequest {
    pub tag: String,
    pub title: String,
}

impl Validate for LanguageDictionaryImportRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_language("tag", Some(&self.tag))?;
        check_title(&self.title)
    }
}

/// The affix and word file of a choice: both chosen, or one of them and
/// the other next to it with the same name.
fn hunspell_pair(chosen: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    let with = |extension: &str| {
        chosen
            .iter()
            .find(|path| {
                path.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(extension))
            })
            .cloned()
    };
    match (with("aff"), with("dic")) {
        (Some(aff), Some(dic)) => Some((aff, dic)),
        (Some(aff), None) => {
            let dic = aff.with_extension("dic");
            dic.is_file().then_some((aff, dic))
        }
        (None, Some(dic)) => {
            let aff = dic.with_extension("aff");
            aff.is_file().then_some((aff, dic))
        }
        (None, None) => None,
    }
}

/// Adds a Hunspell dictionary as a language; none when the dialog was
/// cancelled.
#[tauri::command]
pub async fn language_dictionary_import(
    state: State<'_, AppState>,
    request: Input<LanguageDictionaryImportRequest>,
) -> Result<Option<LanguageDictionary>, CommandError> {
    let request = request.into_inner();
    let answer = state.language_files.pick_files(FilePickRequest {
        title: request.title,
        start_in: None,
        file_name: None,
        filters: vec![FileFilter {
            name: "Hunspell".to_owned(),
            extensions: vec!["aff".to_owned(), "dic".to_owned()],
        }],
    });
    let Ok(Some(chosen)) = answer.await else {
        return Ok(None);
    };
    let (aff, dic) = hunspell_pair(&chosen)
        .ok_or_else(|| CommandError::new(ErrorCode::LanguageDictionaryInvalid))?;
    let service = state.language.clone();
    let tag = request.tag;
    let result =
        tauri::async_runtime::spawn_blocking(move || service.store().import(&tag, &aff, &dic))
            .await
            .map_err(|_| CommandError::new(ErrorCode::Internal))?;
    result.map(Some).map_err(|error| match error {
        ImportError::BadTag => CommandError::invalid_input("tag", "not a language tag"),
        ImportError::Io(error) => {
            log::warn!("dictionary import failed: {error}");
            CommandError::new(ErrorCode::FileSystem)
        }
        other => CommandError::new(ErrorCode::LanguageDictionaryInvalid)
            .with_detail("reason", other.to_string()),
    })
}

/// Request of `language_dictionary_remove`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageTagRequest {
    pub tag: String,
}

impl Validate for LanguageTagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_language("tag", Some(&self.tag))
    }
}

/// Removes an added language dictionary.
#[tauri::command]
pub async fn language_dictionary_remove(
    state: State<'_, AppState>,
    request: Input<LanguageTagRequest>,
) -> Result<(), CommandError> {
    let tag = request.into_inner().tag;
    let service = state.language.clone();
    let result = tauri::async_runtime::spawn_blocking(move || service.store().remove(&tag))
        .await
        .map_err(|_| CommandError::new(ErrorCode::Internal))?;
    result.map_err(|error| match error {
        ImportError::Bundled => CommandError::new(ErrorCode::LanguageDictionaryBundled),
        ImportError::NotFound => CommandError::new(ErrorCode::LanguageNoDictionary),
        ImportError::BadTag => CommandError::invalid_input("tag", "not a language tag"),
        other => {
            log::warn!("dictionary not removed: {other}");
            CommandError::new(ErrorCode::FileSystem)
        }
    })
}

fn tool_error(error: SetupError) -> CommandError {
    match error {
        SetupError::NotLanguageTool => CommandError::new(ErrorCode::LanguageToolNotFound),
        SetupError::NotSaved => CommandError::new(ErrorCode::SettingsNotSaved),
    }
}

/// The state of the LanguageTool add-on; starts it when it is set up and
/// switched on ("bei Bedarf").
#[tauri::command]
pub async fn languagetool_status(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<LanguageToolStatus, CommandError> {
    state.language_tool.ensure_started();
    Ok(state.language_tool.status())
}

/// Request of `languagetool_choose`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageToolChooseRequest {
    pub title: String,
}

impl Validate for LanguageToolChooseRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check_title(&self.title)
    }
}

/// Lets the user choose an existing LanguageTool installation (the folder
/// with `languagetool-server.jar`); none when the dialog was cancelled.
#[tauri::command]
pub async fn languagetool_choose(
    state: State<'_, AppState>,
    request: Input<LanguageToolChooseRequest>,
) -> Result<Option<LanguageToolStatus>, CommandError> {
    let answer = state.language_folders.pick_folder(PickRequest {
        title: request.into_inner().title,
        start_in: None,
    });
    let Ok(Some(folder)) = answer.await else {
        return Ok(None);
    };
    let status = state.language_tool.configure(&folder).map_err(tool_error)?;
    state.language_tool.ensure_started();
    Ok(Some(status))
}

/// Request of `languagetool_set_enabled`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LanguageToolEnabledRequest {
    pub enabled: bool,
}

impl Validate for LanguageToolEnabledRequest {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

#[tauri::command]
pub async fn languagetool_set_enabled(
    state: State<'_, AppState>,
    request: Input<LanguageToolEnabledRequest>,
) -> Result<LanguageToolStatus, CommandError> {
    let status = state
        .language_tool
        .set_enabled(request.into_inner().enabled)
        .map_err(tool_error)?;
    state.language_tool.ensure_started();
    let _ = status;
    Ok(state.language_tool.status())
}

/// Forgets the installation; its files stay.
#[tauri::command]
pub async fn languagetool_forget(
    state: State<'_, AppState>,
    _request: Input<EmptyRequest>,
) -> Result<LanguageToolStatus, CommandError> {
    state.language_tool.forget().map_err(tool_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hunspell_pair_is_found_from_one_or_both_files() {
        let folder = tempfile::tempdir().unwrap();
        let aff = folder.path().join("x.aff");
        let dic = folder.path().join("x.dic");
        std::fs::write(&aff, "SET UTF-8\n").unwrap();
        std::fs::write(&dic, "1\nWort\n").unwrap();
        assert_eq!(
            hunspell_pair(&[dic.clone(), aff.clone()]),
            Some((aff.clone(), dic.clone()))
        );
        assert_eq!(
            hunspell_pair(std::slice::from_ref(&aff)),
            Some((aff.clone(), dic.clone()))
        );
        assert_eq!(
            hunspell_pair(std::slice::from_ref(&dic)),
            Some((aff, dic.clone()))
        );
        std::fs::remove_file(folder.path().join("x.aff")).unwrap();
        assert_eq!(hunspell_pair(&[dic]), None);
    }

    #[test]
    fn check_requests_are_limited() {
        let request = LanguageCheckRequest {
            document_id: Some("not-an-id".to_owned()),
            texts: Vec::new(),
        };
        assert!(request.validate().is_err());
        let request = LanguageCheckRequest {
            document_id: None,
            texts: vec![CheckTextRequest {
                text: "Text".to_owned(),
                language: Some("../de".to_owned()),
                inherit: None,
            }],
        };
        assert!(request.validate().is_err());
        let request = LanguageCheckRequest {
            document_id: None,
            texts: vec![CheckTextRequest {
                text: "x".repeat(MAX_TEXT_CHARS + 1),
                language: None,
                inherit: None,
            }],
        };
        assert!(request.validate().is_err());
    }
}
