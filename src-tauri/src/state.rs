//! Application state shared by all commands.

use std::sync::{Arc, Mutex};

use crate::clock::{Generators, Timestamp};
use crate::database::{Database, DatabaseError};
use crate::services::dialog::{FilePicker, FolderPicker};
use crate::services::extensions::{self, Extensions};
use crate::services::external::ExternalFiles;
use crate::services::language::LanguageService;
use crate::services::language::dictionaries::DictionaryStore;
use crate::services::language::languagetool::LanguageTool;
use crate::services::links::LinkOpener;
use crate::services::locale;
use crate::services::opening::OpenQueue;
use crate::services::session::{AppDirs, LibrarySession};
use crate::services::settings::app::AppSettingsStore;
use crate::services::settings::transfer::SettingsTransfer;
use crate::services::ui::UiService;

pub struct AppState {
    pub session: LibrarySession,
    pub ui: UiService,
    pub picker: Arc<dyn FolderPicker>,
    /// Open and save dialogs for external files.
    pub files: Arc<dyn FilePicker>,
    /// Dialogs of the media import (ADR 0019): the system dialogs, in e2e
    /// builds the answers of `PW_E2E_PICK_MEDIA`.
    pub media_files: Arc<dyn FilePicker>,
    pub media_folders: Arc<dyn FolderPicker>,
    /// Dialogs of export, packages and backups (ADR 0021): the system
    /// dialogs, in e2e builds the answers of `PW_E2E_PICK_EXPORT`.
    pub export_files: Arc<dyn FilePicker>,
    pub export_folders: Arc<dyn FolderPicker>,
    /// External files open in this session.
    pub external: Arc<ExternalFiles>,
    /// Files from start arguments and second starts, until the interface
    /// takes them.
    pub opening: OpenQueue,
    /// Opens web links (`link_open`).
    pub links: Arc<dyn LinkOpener>,
    /// The language check (section 14): dictionaries and engines, for all
    /// libraries.
    pub language: Arc<LanguageService>,
    /// Dialogs of the language check (own word lists, Hunspell files).
    pub language_files: Arc<dyn FilePicker>,
    /// The LanguageTool add-on (ADR 0026) and its folder dialog.
    pub language_tool: Arc<LanguageTool>,
    pub language_folders: Arc<dyn FolderPicker>,
    /// The registries of document formats, exporters and checking engines
    /// (ADR 0027): built once, immutable.
    pub extensions: Arc<Extensions>,
    /// Export, import and reset of the settings (ADR 0027) and their
    /// dialogs: the system dialogs, in e2e builds the answers of
    /// `PW_E2E_PICK_SETTINGS`.
    pub settings_transfer: SettingsTransfer,
    pub settings_files: Arc<dyn FilePicker>,
    /// In-memory database for the system check while no library is open
    /// (the bundled SQLite is checked either way).
    probe: Database,
}

impl AppState {
    /// Loads the app settings and opens the library they name, if any.
    pub async fn initialize(
        dirs: AppDirs,
        picker: Arc<dyn FolderPicker>,
        files: Arc<dyn FilePicker>,
        links: Arc<dyn LinkOpener>,
        generators: Generators,
    ) -> Result<Self, DatabaseError> {
        // The registries first: everything below may look a format up.
        let extensions = Arc::clone(extensions::shared());
        let probe = Database::open_in_memory().await?;
        probe.migrate().await?;
        let (settings, outcome) = AppSettingsStore::load(&dirs.config, Timestamp::now());
        let settings = Arc::new(Mutex::new(settings));
        let ui = UiService::new(Arc::clone(&settings), locale::system_locale(), &dirs.config);
        let external = Arc::new(ExternalFiles::new(
            Arc::clone(&settings),
            generators.clone(),
        ));
        let language = Arc::new(LanguageService::new(Arc::new(DictionaryStore::new(
            dirs.dictionaries.clone(),
            dirs.config.join("dictionaries"),
        ))));
        let language_tool = Arc::new(LanguageTool::new(Arc::clone(&settings)));
        let session = LibrarySession::new(dirs, generators, settings, &outcome);
        session.open_configured().await;
        Ok(Self {
            session,
            ui,
            media_files: Arc::clone(&files),
            media_folders: Arc::clone(&picker),
            export_files: Arc::clone(&files),
            export_folders: Arc::clone(&picker),
            language_files: Arc::clone(&files),
            language_tool,
            language_folders: Arc::clone(&picker),
            settings_transfer: SettingsTransfer::default(),
            settings_files: Arc::clone(&files),
            language,
            picker,
            files,
            external,
            opening: OpenQueue::default(),
            links,
            extensions,
            probe,
        })
    }

    /// The database the system check examines: the open library's, or the
    /// in-memory probe.
    pub async fn check_database(&self) -> Database {
        self.session
            .database()
            .await
            .unwrap_or_else(|| self.probe.clone())
    }
}
