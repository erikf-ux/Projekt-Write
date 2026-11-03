//! Library-wide settings in `application_settings` (section 9).

mod common;

use project_write_lib::clock::Generators;
use project_write_lib::filesystem::location::StorageLocation;
use project_write_lib::services::library::{Library, OpenOptions};
use project_write_lib::services::settings::library::{AcceptedLocation, SettingsError};

#[tokio::test]
async fn settings_are_typed_validated_and_persistent() {
    let dir = common::temp_dir("pw-settings-");
    let path = dir.path().join("lib");
    let (library, _) = Library::create(&path, Generators::system()).await.unwrap();
    let settings = library.settings();

    assert_eq!(settings.get::<AcceptedLocation>().await.unwrap(), None);
    let onedrive = StorageLocation::SyncFolder {
        provider: "OneDrive".to_owned(),
    };
    settings
        .set::<AcceptedLocation>(&Some(onedrive.clone()))
        .await
        .unwrap();
    assert_eq!(
        settings.get::<AcceptedLocation>().await.unwrap(),
        Some(onedrive.clone())
    );
    assert!(matches!(
        settings
            .set::<AcceptedLocation>(&Some(StorageLocation::Local))
            .await,
        Err(SettingsError::Invalid {
            key: "storage.acceptedLocation",
            ..
        })
    ));

    // A key of a newer version stays untouched.
    sqlx::query(
        "INSERT INTO application_settings (key, value, updated_at) \
         VALUES ('future.setting', '{\"x\": 1}', '2030-01-01T00:00:00.000Z')",
    )
    .execute(library.database().writer())
    .await
    .unwrap();
    library.close().await;

    let (library, _) = Library::open(&path, OpenOptions::default(), Generators::system())
        .await
        .unwrap();
    let settings = library.settings();
    assert_eq!(
        settings.get::<AcceptedLocation>().await.unwrap(),
        Some(onedrive)
    );
    settings.reset::<AcceptedLocation>().await.unwrap();
    assert_eq!(settings.get::<AcceptedLocation>().await.unwrap(), None);
    let future: String =
        sqlx::query_scalar("SELECT value FROM application_settings WHERE key = 'future.setting'")
            .fetch_one(library.database().reader())
            .await
            .unwrap();
    assert_eq!(future, "{\"x\": 1}");
    library.close().await;
}

#[tokio::test]
async fn an_unusable_stored_value_yields_the_default_and_is_kept() {
    let dir = common::temp_dir("pw-settings-");
    let (library, _) = Library::create(&dir.path().join("lib"), Generators::system())
        .await
        .unwrap();
    for stored in ["\"no location\"", "{\"kind\": \"local\"}", "42"] {
        sqlx::query(
            "INSERT OR REPLACE INTO application_settings (key, value, updated_at) \
             VALUES ('storage.acceptedLocation', ?1, '2026-11-03T10:00:00.000Z')",
        )
        .bind(stored)
        .execute(library.database().writer())
        .await
        .unwrap();
        assert_eq!(
            library.settings().get::<AcceptedLocation>().await.unwrap(),
            None,
            "{stored}"
        );
        let kept: String = sqlx::query_scalar(
            "SELECT value FROM application_settings WHERE key = 'storage.acceptedLocation'",
        )
        .fetch_one(library.database().reader())
        .await
        .unwrap();
        assert_eq!(kept, stored);
    }
    library.close().await;
}
