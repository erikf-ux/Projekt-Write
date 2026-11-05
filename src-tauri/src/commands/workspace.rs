//! Session commands (stage 1, "Sitzung"; ADR 0016, decision 10): the open
//! tabs, the active tab and the selection in each tab, saved in
//! `settings.json` and opened again after a normal restart. External files
//! are kept by path in Rust; the frontend sends their session ids.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::error::CommandError;
use crate::services::external::ExternalFileInfo;
use crate::services::settings::app::{MAX_SESSION_TABS, SavedSession, SavedTab, SavedTabTarget};
use crate::state::AppState;

/// What a tab shows.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub enum SessionTabTarget {
    /// A library document by id.
    Library { id: String },
    /// An external file by its session id.
    External { id: String },
}

/// The view of a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TabView {
    Rich,
    Source,
    Plain,
}

impl TabView {
    fn as_str(self) -> &'static str {
        match self {
            TabView::Rich => "rich",
            TabView::Source => "source",
            TabView::Plain => "plain",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [TabView::Rich, TabView::Source, TabView::Plain]
            .into_iter()
            .find(|view| view.as_str() == value)
    }
}

/// One tab of `session_save`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SessionTabRequest {
    pub target: SessionTabTarget,
    /// Selection in the editor (ProseMirror positions).
    pub anchor: u32,
    pub head: u32,
    pub view: TabView,
}

/// Request of `session_save`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SaveSessionRequest {
    pub tabs: Vec<SessionTabRequest>,
    /// Index of the active tab.
    pub active: Option<u32>,
}

impl Validate for SaveSessionRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.tabs.len() > MAX_SESSION_TABS {
            return Err(CommandError::invalid_input("tabs", "too many tabs"));
        }
        for tab in &self.tabs {
            match &tab.target {
                SessionTabTarget::Library { id } => {
                    Uuid::try_parse(id)
                        .ok()
                        .filter(|uuid| uuid.hyphenated().to_string() == *id)
                        .ok_or_else(|| CommandError::invalid_input("id", "not a document id"))?;
                }
                SessionTabTarget::External { id } => {
                    let valid = id.len() == 32
                        && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
                    if !valid {
                        return Err(CommandError::invalid_input("id", "not a file id"));
                    }
                }
            }
        }
        if let Some(active) = self.active
            && active as usize >= self.tabs.len()
        {
            return Err(CommandError::invalid_input("active", "no such tab"));
        }
        Ok(())
    }
}

/// A tab to open again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RestoredTabTarget {
    Library { id: String },
    External { file: ExternalFileInfo },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoredTab {
    pub target: RestoredTabTarget,
    pub anchor: u32,
    pub head: u32,
    pub view: TabView,
}

/// Result of `session_restore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoredSession {
    pub tabs: Vec<RestoredTab>,
    pub active: Option<u32>,
}

/// Saves the tabs of this session (2 s after they change, and before
/// quitting). External files that are not open are left out.
#[tauri::command]
pub async fn session_save(
    state: State<'_, AppState>,
    request: Input<SaveSessionRequest>,
) -> Result<(), CommandError> {
    let request = request.into_inner();
    let Some(library_id) = state.session.library_id().await else {
        return Ok(());
    };
    let mut tabs = Vec::new();
    let mut active = None;
    for (index, tab) in request.tabs.into_iter().enumerate() {
        let target = match tab.target {
            SessionTabTarget::Library { id } => SavedTabTarget::Library { id },
            SessionTabTarget::External { id } => match state.external.path(&id) {
                Ok(path) => match path.to_str() {
                    Some(path) => SavedTabTarget::External {
                        path: path.to_owned(),
                    },
                    None => continue,
                },
                Err(_) => continue,
            },
        };
        if request.active == u32::try_from(index).ok() {
            active = u32::try_from(tabs.len()).ok();
        }
        tabs.push(SavedTab {
            target,
            anchor: tab.anchor,
            head: tab.head,
            view: tab.view.as_str().to_owned(),
        });
    }
    state.session.save_tabs(Some(SavedSession {
        library_id,
        tabs,
        active,
    }));
    Ok(())
}

/// The tabs of the last session in the open library, to open again.
/// Documents that are gone and files that vanished are left out.
#[tauri::command]
pub async fn session_restore(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<RestoredSession, CommandError> {
    let _ = request.into_inner();
    let empty = RestoredSession {
        tabs: Vec::new(),
        active: None,
    };
    let (Some(library_id), Some(saved)) =
        (state.session.library_id().await, state.session.saved_tabs())
    else {
        return Ok(empty);
    };
    if saved.library_id != library_id {
        return Ok(empty);
    }
    let mut tabs = Vec::new();
    let mut active = None;
    for (index, tab) in saved.tabs.into_iter().enumerate() {
        let target = match tab.target {
            SavedTabTarget::Library { id } => {
                let Ok(uuid) = Uuid::try_parse(&id) else {
                    continue;
                };
                let exists = state
                    .session
                    .with_documents(|documents| async move {
                        Ok(documents
                            .get(uuid)
                            .await
                            .ok()
                            .flatten()
                            .is_some_and(|meta| meta.deleted_at.is_none()))
                    })
                    .await
                    .unwrap_or(false);
                if !exists {
                    continue;
                }
                RestoredTabTarget::Library { id }
            }
            SavedTabTarget::External { path } => match state.external.register(Path::new(&path)) {
                Ok(file) => RestoredTabTarget::External { file },
                Err(_) => continue,
            },
        };
        if saved.active == u32::try_from(index).ok() {
            active = u32::try_from(tabs.len()).ok();
        }
        tabs.push(RestoredTab {
            target,
            anchor: tab.anchor,
            head: tab.head,
            view: TabView::parse(&tab.view).unwrap_or(TabView::Rich),
        });
    }
    Ok(RestoredSession { tabs, active })
}
