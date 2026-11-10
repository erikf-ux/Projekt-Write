//! Migrations (section 18): "nur vorwärts, in einer Transaktion, mit
//! Sicherung vorab und getestet ab jeder früheren Schemaversion.
//! Ausgelieferte Migrationen werden nie geändert."
//!
//! The backup before migrating is tested with the library (tests/library.rs).
//!
//! When a migration is added: add a fixture row for its tables to
//! [`FIXTURES`] and its checksum to [`SHIPPED`].

mod common;

use std::borrow::Cow;

use project_write_lib::database::{Database, JournalMode, MIGRATOR, diagnostics};
use sqlx::migrate::{Migration, MigrationType, Migrator};
use sqlx::{AssertSqlSafe, Row, SqlStr};

/// Per schema version: a statement that stores data in the tables of that
/// version, and a query that counts it (must return 1 after every upgrade).
const FIXTURES: &[(i64, &str, &str)] = &[
    (
        1,
        "INSERT INTO application_settings (key, value, updated_at) \
         VALUES ('fixture.v1', '{\"a\": [1, 2]}', '2026-11-01T10:00:00.000Z')",
        "SELECT count(*) FROM application_settings \
         WHERE key = 'fixture.v1' AND value = '{\"a\": [1, 2]}'",
    ),
    (
        2,
        "INSERT INTO documents (id, title, format, size_bytes, content_sha256, created_at, updated_at) \
         VALUES ('0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11', 'Fixture', 'md', 3, \
                 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', \
                 '2026-11-02T10:00:00.000Z', '2026-11-02T10:00:00.000Z')",
        "SELECT count(*) FROM documents \
         WHERE id = '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11' AND title = 'Fixture'",
    ),
    (
        3,
        "INSERT INTO documents (id, title, format, size_bytes, content_sha256, created_at, updated_at, original_name) \
         VALUES ('5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', 'Notiz', 'txt', 0, \
                 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', \
                 '2026-11-03T10:00:00.000Z', '2026-11-03T10:00:00.000Z', 'Notiz.txt')",
        "SELECT count(*) FROM documents \
         WHERE id = '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f' AND original_name = 'Notiz.txt'",
    ),
    (
        4,
        "UPDATE documents SET favorited_at = '2026-11-04T10:00:00.000Z', \
                              last_opened_at = '2026-11-04T10:01:00.000Z' \
         WHERE id = '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f'",
        "SELECT count(*) FROM documents \
         WHERE id = '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f' \
           AND favorited_at = '2026-11-04T10:00:00.000Z' \
           AND last_opened_at = '2026-11-04T10:01:00.000Z'",
    ),
    (
        5,
        "INSERT INTO document_links (source_id, target_id) \
         VALUES ('5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11')",
        "SELECT count(*) FROM document_links \
         WHERE source_id = '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f' \
           AND target_id = '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11'",
    ),
    (
        6,
        "INSERT INTO document_history (id, document_id, kind, name, format, content, compression, size_bytes, content_sha256, created_at) \
         VALUES ('9a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d', '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', 'named', 'Erste Fassung', \
                 'txt', X'0300', 'deflate', 0, \
                 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', '2026-11-05T10:00:00.000Z')",
        "SELECT count(*) FROM document_history \
         WHERE id = '9a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d' AND name = 'Erste Fassung' AND content = X'0300'",
    ),
    (
        7,
        "INSERT INTO subnets (id, name, name_key, description, color, parent_id, created_at, updated_at) \
         VALUES ('7a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d', 'Projekte', 'projekte', 'Alles Laufende', 'blue', NULL, \
                 '2026-11-05T11:00:00.000Z', '2026-11-05T11:00:00.000Z'); \
         INSERT INTO subnet_memberships (subnet_id, document_id, is_primary, created_at) \
         VALUES ('7a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d', '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', 1, '2026-11-05T11:00:00.000Z'); \
         INSERT INTO tags (id, name, name_key, created_at, updated_at) \
         VALUES ('8b2c3d4e-5f6a-4b7c-9d8e-1f2a3b4c5d6e', 'Äpfel', 'äpfel', '2026-11-05T11:00:00.000Z', '2026-11-05T11:00:00.000Z'); \
         INSERT INTO document_tags (document_id, tag_id, created_at) \
         VALUES ('5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', '8b2c3d4e-5f6a-4b7c-9d8e-1f2a3b4c5d6e', '2026-11-05T11:00:00.000Z')",
        "SELECT count(*) FROM subnet_memberships AS m \
         JOIN subnets AS s ON s.id = m.subnet_id \
         JOIN document_tags AS d ON d.document_id = m.document_id \
         JOIN tags AS t ON t.id = d.tag_id \
         WHERE s.name = 'Projekte' AND m.is_primary = 1 AND t.name = 'Äpfel'",
    ),
    (
        8,
        "INSERT INTO media (id, kind, mime_type, extension, title, original_name, import_folder, size_bytes, sha256, \
                            width, height, created_at, updated_at) \
         VALUES ('6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f', 'image', 'image/png', 'png', 'Skizze', 'skizze.png', \
                 'Bilder', 67, 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 1, 1, \
                 '2026-11-10T10:00:00.000Z', '2026-11-10T10:00:00.000Z'); \
         INSERT INTO subnet_memberships (subnet_id, media_id, is_primary, created_at) \
         VALUES ('7a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d', '6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f', 1, '2026-11-10T10:00:00.000Z'); \
         INSERT INTO media_tags (media_id, tag_id, created_at) \
         VALUES ('6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f', '8b2c3d4e-5f6a-4b7c-9d8e-1f2a3b4c5d6e', '2026-11-10T10:00:00.000Z'); \
         INSERT INTO document_media (document_id, media_id, page) \
         VALUES ('5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f', '6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f', 0)",
        "SELECT count(*) FROM media AS x \
         JOIN subnet_memberships AS m ON m.media_id = x.id AND m.document_id IS NULL \
         JOIN media_tags AS g ON g.media_id = x.id \
         JOIN document_media AS d ON d.media_id = x.id \
         WHERE x.title = 'Skizze' AND m.is_primary = 1 AND d.page = 0",
    ),
];

/// Version, description and SHA-384 checksum (hex) of every shipped
/// migration. A change to a shipped file fails this test.
const SHIPPED: &[(i64, &str, &str)] = &[
    (
        1,
        "application settings",
        "d30b6db00247c5485340f37f0aeeaf582d4bf7016364ccccbab8e07e3d56159f05159155aac14b725ca209f2d815fe83",
    ),
    (
        2,
        "documents",
        "f99379f2303d326603d0aabc2fcb0a41be5675dc26656111b6164d9489d99f210efb0fe079ad6677ac8371f8438ad477",
    ),
    (
        3,
        "document import",
        "f5a8e5b46bdd724029e64b2c3d4c49bb5c643b0bd2b666df0472f830da5d9195d945ecebe43f59e4de45dcf6deb726cb",
    ),
    (
        4,
        "favorites and recent",
        "522b96e675abf0d081cbe541dd41eb5044c322af46435b69d1265f7147f255461a43c4222b949270c379cf5c877a4aba",
    ),
    (
        5,
        "document links",
        "e584d60dfe05ea529c3fb0602cf57ea10b1c32c91cdd321819a2cea6d3735f593e3ed685f669ddd3b7c96871c4ee07e7",
    ),
    (
        6,
        "document history",
        "546edd56b24d253ac981b4ad7d7e8ff14d730cd452e17171321e1bdbe51f2166f770c0ca6415a7c59e5f5ef86aed6974",
    ),
    (
        7,
        "subnets and tags",
        "b8eb8bd196ae4b39c4c9c5d7cb3c294587e1e3acc8dfdd3215bb5779c0edb394afd6c2c24fb51c0e83b7cebd7f986536",
    ),
    (
        8,
        "media",
        "874c21ba69336c1a9fe3cdc69e497a66fc25cfcf4be418082a9e5c362ad3b046ea1c214a085908b44d40173d2d8708a2",
    ),
];

async fn fresh(dir: &tempfile::TempDir, name: &str) -> Database {
    Database::open_file(&dir.path().join(name), JournalMode::Wal)
        .await
        .unwrap()
}

/// All schema objects (tables, indexes, triggers, views) with their SQL.
async fn schema_objects(db: &Database) -> Vec<String> {
    sqlx::query(
        "SELECT type, name, tbl_name, coalesce(sql, '') FROM sqlite_schema ORDER BY type, name",
    )
    .fetch_all(db.reader())
    .await
    .unwrap()
    .iter()
    .map(|row| {
        format!(
            "{}|{}|{}|{}",
            row.get::<String, _>(0),
            row.get::<String, _>(1),
            row.get::<String, _>(2),
            row.get::<String, _>(3)
        )
    })
    .collect()
}

#[tokio::test]
async fn every_earlier_schema_version_upgrades_to_the_current_one() {
    let dir = common::temp_dir("pw-migrate-");
    let latest = Database::supported_version();
    assert_eq!(
        FIXTURES.iter().map(|f| f.0).max(),
        Some(latest),
        "every migration needs a fixture"
    );

    let reference = fresh(&dir, "reference.db").await;
    reference.migrate().await.unwrap();
    let expected_schema = schema_objects(&reference).await;
    reference.close().await;

    for start in 0..latest {
        let db = fresh(&dir, &format!("from-{start}.db")).await;
        if start > 0 {
            db.migrate_to(start).await.unwrap();
        }
        assert_eq!(db.schema_version().await.unwrap(), start);
        for (version, insert, _) in FIXTURES.iter().filter(|f| f.0 <= start) {
            // A fixture may hold several statements (raw_sql runs them all).
            sqlx::raw_sql(AssertSqlSafe(*insert))
                .execute(db.writer())
                .await
                .unwrap_or_else(|e| panic!("fixture {version} at schema {start}: {e}"));
        }

        let report = db.migrate().await.unwrap();
        assert_eq!(report.applied, ((start + 1)..=latest).collect::<Vec<_>>());
        assert_eq!(report.version, latest);
        assert_eq!(schema_objects(&db).await, expected_schema, "from {start}");
        for (version, _, check) in FIXTURES.iter().filter(|f| f.0 <= start) {
            let count: i64 = sqlx::query_scalar(AssertSqlSafe(*check))
                .fetch_one(db.reader())
                .await
                .unwrap();
            assert_eq!(
                count, 1,
                "data of schema {version} after upgrading from {start}"
            );
        }
        assert!(diagnostics::integrity_check(db.reader()).await.unwrap());
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(db.reader())
            .await
            .unwrap();
        assert!(violations.is_empty(), "foreign key violations from {start}");
        db.close().await;
    }
}

#[test]
fn shipped_migrations_are_never_changed() {
    let actual: Vec<(i64, String, String)> = MIGRATOR
        .iter()
        .map(|m| {
            let checksum: String = m.checksum.iter().map(|b| format!("{b:02x}")).collect();
            (m.version, m.description.to_string(), checksum)
        })
        .collect();
    let expected: Vec<(i64, String, String)> = SHIPPED
        .iter()
        .map(|(v, d, c)| (*v, (*d).to_owned(), (*c).to_owned()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn every_migration_runs_in_a_transaction() {
    for migration in MIGRATOR.iter() {
        assert!(
            !migration.no_tx,
            "migration {} opts out of the transaction",
            migration.version
        );
        assert_eq!(migration.migration_type, MigrationType::Simple);
    }
}

#[tokio::test]
async fn a_failing_migration_changes_nothing() {
    let dir = common::temp_dir("pw-migrate-");
    let db = fresh(&dir, "failing.db").await;
    db.migrate().await.unwrap();
    let before = schema_objects(&db).await;

    // A migration whose second statement fails.
    let mut migrations: Vec<Migration> = MIGRATOR.iter().cloned().collect();
    let next = Database::supported_version() + 1;
    migrations.push(Migration::new(
        next,
        Cow::Borrowed("broken"),
        MigrationType::Simple,
        SqlStr::from_static(
            "CREATE TABLE half_done (id TEXT NOT NULL PRIMARY KEY) STRICT; \
             INSERT INTO missing_table VALUES (1);",
        ),
        false,
    ));
    let migrator = Migrator::with_migrations(migrations);
    assert!(migrator.run(db.writer()).await.is_err());

    assert_eq!(schema_objects(&db).await, before);
    assert_eq!(
        db.schema_version().await.unwrap(),
        Database::supported_version()
    );
    db.close().await;
}
