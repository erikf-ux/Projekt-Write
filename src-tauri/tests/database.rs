//! Database acceptance tests for Phase 1 (section 25):
//! migrations, foreign keys, pragmas and FTS5.
//!
//! All databases live in temporary folders inside `target/` – never outside
//! the project folder (section 2).

use project_write_lib::database::{Database, DatabaseError, JournalMode, diagnostics, fts};
use sqlx::{AssertSqlSafe, Row};

fn temp_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("pw-db-test-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("create temp dir inside target/")
}

async fn open(dir: &tempfile::TempDir, journal: JournalMode) -> Database {
    Database::open_file(&dir.path().join("projectwrite.db"), journal)
        .await
        .expect("open database")
}

/// Everything that describes the schema: objects and recorded migrations.
async fn schema_snapshot(db: &Database) -> Vec<String> {
    let mut snapshot: Vec<String> =
        sqlx::query("SELECT type, name, coalesce(sql, '') FROM sqlite_schema ORDER BY type, name")
            .fetch_all(db.reader())
            .await
            .unwrap()
            .iter()
            .map(|row| {
                format!(
                    "{}|{}|{}",
                    row.get::<String, _>(0),
                    row.get::<String, _>(1),
                    row.get::<String, _>(2)
                )
            })
            .collect();
    let migrations = sqlx::query("SELECT version, description, success, hex(checksum), installed_on FROM _sqlx_migrations ORDER BY version")
        .fetch_all(db.reader())
        .await
        .unwrap();
    for row in migrations {
        snapshot.push(format!(
            "migration|{}|{}|{}|{}|{}",
            row.get::<i64, _>(0),
            row.get::<String, _>(1),
            row.get::<bool, _>(2),
            row.get::<String, _>(3),
            row.get::<String, _>(4)
        ));
    }
    snapshot
}

#[tokio::test]
async fn migration_runs_on_empty_database_and_second_run_changes_nothing() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    assert_eq!(db.schema_version().await.unwrap(), 0);

    let first = db.migrate().await.unwrap();
    assert_eq!(
        first.applied,
        (1..=Database::supported_version()).collect::<Vec<_>>()
    );
    assert_eq!(first.version, Database::supported_version());
    let after_first = schema_snapshot(&db).await;
    for table in ["application_settings", "documents"] {
        assert!(
            after_first
                .iter()
                .any(|line| line.starts_with(&format!("table|{table}|"))),
            "{table}"
        );
    }

    let second = db.migrate().await.unwrap();
    assert!(
        second.applied.is_empty(),
        "second run applied {:?}",
        second.applied
    );
    assert_eq!(schema_snapshot(&db).await, after_first);
    db.close().await;

    // Reopening and migrating again changes nothing either.
    let db = open(&dir, JournalMode::Wal).await;
    let third = db.migrate().await.unwrap();
    assert!(third.applied.is_empty());
    assert_eq!(schema_snapshot(&db).await, after_first);
    assert!(diagnostics::integrity_check(db.reader()).await.unwrap());
    assert!(diagnostics::quick_check(db.reader()).await.unwrap());
    db.close().await;
}

#[tokio::test]
async fn tables_are_strict() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    db.migrate().await.unwrap();
    let non_strict: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'table' \
         AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' AND strict = 0",
    )
    .fetch_all(db.reader())
    .await
    .unwrap();
    assert!(
        non_strict.is_empty(),
        "tables without STRICT: {non_strict:?}"
    );
    db.close().await;
}

#[tokio::test]
async fn foreign_keys_are_enabled_and_enforced() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    db.migrate().await.unwrap();
    assert!(
        diagnostics::foreign_keys_enabled(db.writer())
            .await
            .unwrap()
    );
    assert!(
        diagnostics::foreign_keys_enabled(db.reader())
            .await
            .unwrap()
    );

    sqlx::query("CREATE TABLE parent (id TEXT PRIMARY KEY) STRICT")
        .execute(db.writer())
        .await
        .unwrap();
    sqlx::query("CREATE TABLE child (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL REFERENCES parent(id)) STRICT")
        .execute(db.writer())
        .await
        .unwrap();
    let orphan = sqlx::query("INSERT INTO child (id, parent_id) VALUES ('c1', 'missing')")
        .execute(db.writer())
        .await;
    let error = orphan.expect_err("orphan row must be rejected");
    assert!(error.to_string().contains("FOREIGN KEY"), "{error}");
    db.close().await;
}

#[tokio::test]
async fn connections_use_the_required_pragmas() {
    for (journal, expected) in [(JournalMode::Wal, "wal"), (JournalMode::Rollback, "delete")] {
        let dir = temp_dir();
        let db = open(&dir, journal).await;
        assert_eq!(
            diagnostics::journal_mode(db.writer()).await.unwrap(),
            expected
        );
        assert_eq!(
            diagnostics::synchronous(db.writer()).await.unwrap(),
            2,
            "synchronous=FULL"
        );
        assert_eq!(
            diagnostics::busy_timeout_ms(db.writer()).await.unwrap(),
            5000
        );
        assert_eq!(diagnostics::synchronous(db.reader()).await.unwrap(), 2);
        db.close().await;
    }
}

#[tokio::test]
async fn reader_connections_are_read_only() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    db.migrate().await.unwrap();
    let write = sqlx::query("INSERT INTO application_settings (key, value, updated_at) VALUES ('a', '1', '2026-11-02T10:00:00Z')")
        .execute(db.reader())
        .await;
    assert!(write.is_err(), "reader must not write");
    db.close().await;
}

#[tokio::test]
async fn settings_table_validates_rows() {
    let db = Database::open_in_memory().await.unwrap();
    db.migrate().await.unwrap();
    let insert = |key: &'static str, value: &'static str, at: &'static str| {
        let pool = db.writer().clone();
        async move {
            sqlx::query(
                "INSERT INTO application_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            )
            .bind(key)
            .bind(value)
            .bind(at)
            .execute(&pool)
            .await
        }
    };
    assert!(
        insert("editor.lineWidth", "72", "2026-11-02T10:00:00Z")
            .await
            .is_ok()
    );
    assert!(
        insert("editor.font", "{\"size\":16}", "2026-11-02T10:00:00.123Z")
            .await
            .is_ok()
    );
    assert!(
        insert("", "1", "2026-11-02T10:00:00Z").await.is_err(),
        "empty key"
    );
    assert!(
        insert("broken", "{not json", "2026-11-02T10:00:00Z")
            .await
            .is_err(),
        "invalid JSON"
    );
    assert!(
        insert("local", "1", "2026-11-02 10:00:00").await.is_err(),
        "not ISO 8601 UTC"
    );
    assert!(
        insert("editor.lineWidth", "80", "2026-11-02T10:00:00Z")
            .await
            .is_err(),
        "duplicate key"
    );
}

#[tokio::test]
async fn fts5_is_compiled_in_and_both_search_tables_work() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    db.migrate().await.unwrap();

    let options = diagnostics::compile_options(db.reader()).await.unwrap();
    assert!(
        options.iter().any(|o| o == "ENABLE_FTS5"),
        "compile options: {options:?}"
    );

    // The exact definitions from section 19, as real tables.
    sqlx::query(AssertSqlSafe(format!(
        "CREATE VIRTUAL TABLE search_words USING fts5({})",
        fts::SEARCH_WORDS_DEFINITION
    )))
    .execute(db.writer())
    .await
    .expect("create search_words");
    sqlx::query(AssertSqlSafe(format!(
        "CREATE VIRTUAL TABLE search_parts USING fts5({})",
        fts::SEARCH_PARTS_DEFINITION
    )))
    .execute(db.writer())
    .await
    .expect("create search_parts");

    let body = "Programmieren in C++ im Krankenhaus mit Herrn Müller";
    sqlx::query("INSERT INTO search_words (title, body, tags, subnets, meta) VALUES ('Notiz', ?1, '', '', '')")
        .bind(body)
        .execute(db.writer())
        .await
        .unwrap();
    sqlx::query("INSERT INTO search_parts (title, body) VALUES ('Notiz', ?1)")
        .bind(body)
        .execute(db.writer())
        .await
        .unwrap();

    let count = |table: &'static str, term: &'static str| {
        let pool = db.reader().clone();
        async move {
            sqlx::query_scalar::<_, i64>(AssertSqlSafe(format!(
                "SELECT count(*) FROM {table} WHERE {table} MATCH ?1"
            )))
            .bind(fts::quote_term(term))
            .fetch_one(&pool)
            .await
        }
    };

    assert_eq!(
        count("search_words", "C++").await.unwrap(),
        1,
        "\"C++\" must work"
    );
    assert_eq!(
        count("search_words", "Muller").await.unwrap(),
        1,
        "remove_diacritics"
    );
    assert_eq!(
        count("search_words", "\"").await.unwrap(),
        0,
        "a lone quote is not an error"
    );
    assert_eq!(
        count("search_parts", "haus").await.unwrap(),
        1,
        "trigram finds word parts"
    );

    // Unquoted, C++ is a syntax error – this is why every term is quoted.
    let unquoted = sqlx::query("SELECT count(*) FROM search_words WHERE search_words MATCH 'C++'")
        .fetch_one(db.reader())
        .await;
    assert!(unquoted.is_err());

    let probe = fts::probe(db.writer()).await.unwrap();
    assert!(probe.passed(), "{probe:?}");
    db.close().await;
}

#[tokio::test]
async fn fts_probe_leaves_no_trace() {
    let db = Database::open_in_memory().await.unwrap();
    db.migrate().await.unwrap();
    assert!(fts::probe(db.writer()).await.unwrap().passed());
    let leftovers: i64 =
        sqlx::query_scalar("SELECT count(*) FROM temp.sqlite_schema WHERE name LIKE 'pw_probe%'")
            .fetch_one(db.writer())
            .await
            .unwrap();
    assert_eq!(leftovers, 0);
}

#[tokio::test]
async fn newer_schema_is_rejected_without_changes() {
    let dir = temp_dir();
    let db = open(&dir, JournalMode::Wal).await;
    db.migrate().await.unwrap();
    // Simulate a database written by a future version of the app.
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (9999, 'from the future', 1, x'00', 0)",
    )
    .execute(db.writer())
    .await
    .unwrap();
    let before = schema_snapshot(&db).await;

    match db.migrate().await {
        Err(DatabaseError::SchemaTooNew { found, supported }) => {
            assert_eq!(found, 9999);
            assert_eq!(supported, Database::supported_version());
        }
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
    assert_eq!(schema_snapshot(&db).await, before);
    db.close().await;
}

#[tokio::test]
async fn in_memory_database_survives_idle_time() {
    let db = Database::open_in_memory().await.unwrap();
    db.migrate().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(
        db.schema_version().await.unwrap(),
        Database::supported_version()
    );
}
