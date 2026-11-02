//! Application state shared by all commands.

use crate::database::{Database, DatabaseError};

pub struct AppState {
    pub database: Database,
}

impl AppState {
    /// Phase 1: there is no library yet (Phase 2), so the database lives in
    /// memory and nothing is written to disk. See ADR 0004.
    pub async fn initialize() -> Result<Self, DatabaseError> {
        let database = Database::open_in_memory().await?;
        database.migrate().await?;
        Ok(Self { database })
    }
}
