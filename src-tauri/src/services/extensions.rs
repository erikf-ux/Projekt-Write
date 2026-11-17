//! The registries of the app in one place (section 22; ADR 0027).
//!
//! [`Extensions::builtin`] lists every built-in entry of every registry in
//! Rust: document formats, exporters and checking engines. It is the only
//! place entries come from. The registries are built once per process
//! ([`shared`]) and never change afterwards: [`crate::services::registry`]
//! has no method that adds, replaces or removes an entry, there is no
//! plugin folder and no command that loads code. `AppState` holds the same
//! instance for the commands.

use std::sync::{Arc, LazyLock};

use serde::Serialize;
use ts_rs::TS;

use crate::services::export::exporters::{self, Exporter};
use crate::services::formats::{self, DocumentFormats};
use crate::services::language::engines::{self, CheckingEngine};
use crate::services::registry::{Registry, RegistryEntry};

/// All registries in Rust.
pub struct Extensions {
    pub document_formats: DocumentFormats,
    pub exporters: Registry<Box<dyn Exporter>>,
    pub language_engines: Registry<Box<dyn CheckingEngine>>,
}

impl Extensions {
    /// Builds the registries from the built-in entries. Panics on a
    /// malformed or repeated id (a programming error, see
    /// [`Registry::new`]).
    pub fn builtin() -> Self {
        Self {
            document_formats: DocumentFormats::new(formats::builtin()),
            exporters: Registry::new("exporters", exporters::builtin()),
            language_engines: Registry::new("languageEngines", engines::builtin()),
        }
    }

    /// Every entry of every registry, for the settings ("Erweitert").
    pub fn list(&self) -> Vec<ExtensionInfo> {
        let formats = self.document_formats.iter().map(|entry| {
            ExtensionInfo::builtin(ExtensionRegistry::DocumentFormats, entry.id(), false)
        });
        let exporters = self
            .exporters
            .iter()
            .map(|entry| ExtensionInfo::builtin(ExtensionRegistry::Exporters, entry.id(), false));
        let engines = self.language_engines.iter().map(|entry| {
            ExtensionInfo::builtin(
                ExtensionRegistry::LanguageEngines,
                entry.id(),
                entry.add_on(),
            )
        });
        formats.chain(exporters).chain(engines).collect()
    }
}

static SHARED: LazyLock<Arc<Extensions>> = LazyLock::new(|| Arc::new(Extensions::builtin()));

/// The registries of this process, built on first use (at the latest when
/// the app state is set up).
pub fn shared() -> &'static Arc<Extensions> {
    &SHARED
}

/// A registry in Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExtensionRegistry {
    DocumentFormats,
    Exporters,
    LanguageEngines,
}

/// Where an entry comes from. Version 1.0 has built-in entries only
/// (section 22: no code of third parties).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExtensionOrigin {
    Builtin,
}

/// One entry of a registry, as `extensions_list` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionInfo {
    pub registry: ExtensionRegistry,
    pub id: String,
    pub origin: ExtensionOrigin,
    /// An add-on the user installs separately (LanguageTool).
    pub add_on: bool,
}

impl ExtensionInfo {
    fn builtin(registry: ExtensionRegistry, id: &str, add_on: bool) -> Self {
        Self {
            registry,
            id: id.to_owned(),
            origin: ExtensionOrigin::Builtin,
            add_on,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registries hold exactly the built-in entries: nothing else can
    /// appear, because entries come only from [`Extensions::builtin`] and a
    /// registry cannot be changed after it is built (ADR 0027, decision 3).
    #[test]
    fn the_registries_hold_exactly_the_built_in_entries() {
        let extensions = shared();
        assert_eq!(
            extensions
                .document_formats
                .registry()
                .ids()
                .collect::<Vec<_>>(),
            ["txt", "md", "pwrite"]
        );
        assert_eq!(
            extensions.exporters.ids().collect::<Vec<_>>(),
            ["txt", "markdown", "pack", "library"]
        );
        assert_eq!(
            extensions.language_engines.ids().collect::<Vec<_>>(),
            ["spelling", "rules", "languagetool"]
        );
    }

    #[test]
    fn the_list_names_every_entry_as_built_in() {
        let list = shared().list();
        assert_eq!(list.len(), 10);
        assert!(
            list.iter()
                .all(|entry| entry.origin == ExtensionOrigin::Builtin)
        );
        let add_ons: Vec<&str> = list
            .iter()
            .filter(|entry| entry.add_on)
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(add_ons, ["languagetool"]);
        assert_eq!(
            serde_json::to_value(&list[0]).unwrap(),
            serde_json::json!({
                "registry": "documentFormats",
                "id": "txt",
                "origin": "builtin",
                "addOn": false
            })
        );
    }

    #[test]
    fn building_again_gives_the_same_registries() {
        assert_eq!(Extensions::builtin().list(), shared().list());
    }
}
