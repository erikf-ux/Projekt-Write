//! Application state shared by all commands.

use std::sync::{Arc, Mutex};

use crate::clock::{Generators, Timestamp};
use crate::database::{Database, DatabaseError};
use crate::services::dialog::FolderPicker;
use crate::services::locale;
use crate::services::session::{AppDirs, LibrarySession};
use crate::services::settings::app::AppSettingsStore;
use crate::services::ui::UiService;

pub struct AppState {
    pub session: LibrarySession,
    pub ui: UiService,
    pub picker: Arc<dyn FolderPicker>,
    /// In-memory database for the system check while no library is open
    /// (the bundled SQLite is checked either way).
    probe: Database,
}

impl AppState {
    /// Loads the app settings and opens the library they name, if any.
    pub async fn initialize(
        dirs: AppDirs,
        picker: Arc<dyn FolderPicker>,
        generators: Generators,
    ) -> Result<Self, DatabaseError> {
        let probe = Database::open_in_memory().await?;
        probe.migrate().await?;
        let (settings, outcome) = AppSettingsStore::load(&dirs.config, Timestamp::now());
        let settings = Arc::new(Mutex::new(settings));
        let ui = UiService::new(Arc::clone(&settings), locale::system_locale(), &dirs.config);
        let session = LibrarySession::new(dirs, generators, settings, &outcome);
        session.open_configured().await;
        Ok(Self {
            session,
            ui,
            picker,
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
