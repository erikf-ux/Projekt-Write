//! System self-check shown on the start screen.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::commands::input::Validate;
use crate::database::{Database, DatabaseError, diagnostics, fts};
use crate::error::CommandError;

/// One check the system status can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SystemCheckId {
    /// The database answers and `PRAGMA quick_check` reports `ok`.
    Database,
    /// The schema is at the version this build expects.
    Migrations,
    /// Foreign keys are enforced on the writer connection.
    ForeignKeys,
    /// FTS5 is compiled in and both search table definitions work.
    FullTextSearch,
}

impl SystemCheckId {
    pub const ALL: [SystemCheckId; 4] = [
        SystemCheckId::Database,
        SystemCheckId::Migrations,
        SystemCheckId::ForeignKeys,
        SystemCheckId::FullTextSearch,
    ];
}

/// Request of the `system_check` command.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SystemCheckRequest {
    /// Checks to run, in display order. Must be non-empty and free of duplicates.
    pub checks: Vec<SystemCheckId>,
}

impl Validate for SystemCheckRequest {
    fn validate(&self) -> Result<(), CommandError> {
        if self.checks.is_empty() {
            return Err(CommandError::invalid_input("checks", "empty"));
        }
        let mut seen = HashSet::new();
        if !self.checks.iter().all(|check| seen.insert(*check)) {
            return Err(CommandError::invalid_input("checks", "duplicate"));
        }
        Ok(())
    }
}

/// Result of a single check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemCheckResult {
    pub check: SystemCheckId,
    pub passed: bool,
}

/// Response of the `system_check` command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemCheckResponse {
    pub app_version: String,
    /// Operating system: `windows`, `macos` or `linux`.
    pub platform: String,
    pub sqlite_version: String,
    #[ts(type = "number")]
    pub schema_version: i64,
    pub results: Vec<SystemCheckResult>,
}

/// Runs the requested checks against `database`.
///
/// A failing check is reported as `passed: false`; only errors that prevent
/// reporting at all (the database does not answer) are returned as `Err`.
pub async fn run_checks(
    database: &Database,
    checks: &[SystemCheckId],
) -> Result<SystemCheckResponse, DatabaseError> {
    let sqlite_version = diagnostics::sqlite_version(database.reader()).await?;
    let schema_version = database.schema_version().await?;

    let mut results = Vec::with_capacity(checks.len());
    for &check in checks {
        let passed = match run_check(database, check).await {
            Ok(passed) => passed,
            Err(error) => {
                log::warn!("system check {check:?} failed: {error}");
                false
            }
        };
        results.push(SystemCheckResult { check, passed });
    }

    Ok(SystemCheckResponse {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
        sqlite_version,
        schema_version,
        results,
    })
}

async fn run_check(database: &Database, check: SystemCheckId) -> Result<bool, DatabaseError> {
    Ok(match check {
        SystemCheckId::Database => diagnostics::quick_check(database.reader()).await?,
        SystemCheckId::Migrations => {
            database.schema_version().await? == Database::supported_version()
        }
        SystemCheckId::ForeignKeys => diagnostics::foreign_keys_enabled(database.writer()).await?,
        SystemCheckId::FullTextSearch => fts::probe(database.writer()).await?.passed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn request(checks: Vec<SystemCheckId>) -> SystemCheckRequest {
        SystemCheckRequest { checks }
    }

    #[test]
    fn validation_accepts_all_checks_once() {
        assert!(request(SystemCheckId::ALL.to_vec()).validate().is_ok());
    }

    #[test]
    fn validation_rejects_empty_and_duplicates() {
        for (checks, reason) in [
            (vec![], "empty"),
            (
                vec![SystemCheckId::Database, SystemCheckId::Database],
                "duplicate",
            ),
        ] {
            let error = request(checks).validate().unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInput);
            assert_eq!(error.details.unwrap()["reason"], reason);
        }
    }

    #[tokio::test]
    async fn all_checks_pass_on_a_migrated_database() {
        let database = Database::open_in_memory().await.unwrap();
        database.migrate().await.unwrap();
        let response = run_checks(&database, &SystemCheckId::ALL).await.unwrap();
        assert_eq!(response.schema_version, Database::supported_version());
        assert_eq!(response.results.len(), SystemCheckId::ALL.len());
        assert!(response.results.iter().all(|r| r.passed), "{response:?}");
    }

    #[tokio::test]
    async fn migration_check_fails_before_migrating() {
        let database = Database::open_in_memory().await.unwrap();
        let response = run_checks(&database, &[SystemCheckId::Migrations])
            .await
            .unwrap();
        assert_eq!(response.schema_version, 0);
        assert!(!response.results[0].passed);
    }
}
