//! Application state shared by all commands.

use std::sync::Arc;

use crate::clock::{Generators, Timestamp};
use crate::database::{Database, DatabaseError};
use crate::services::dialog::FolderPicker;
use crate::services::session::{AppDirs, LibrarySession};
use crate::services::settings::app::AppSettingsStore;

pub struct AppState {
    pub session: LibrarySession,
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
        let session = LibrarySession::new(dirs, generators, settings, &outcome);
        session.open_configured().await;
        Ok(Self {
            session,
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
