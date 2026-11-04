//! IPC contract tests through Tauri's real command pipeline (mock runtime):
//! a valid request yields the typed result, every invalid request yields the
//! uniform error format `{ code, messageKey, details? }` (section 8).
//!
//! Library commands run against temporary folders inside `target/`; the
//! folder dialog is replaced by a scripted one.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use project_write_lib::clock::Generators;
use project_write_lib::commands;
use project_write_lib::database::Database;
use project_write_lib::services::dialog::ScriptedPicker;
use project_write_lib::services::links::RecordingOpener;
use project_write_lib::services::session::AppDirs;
use project_write_lib::state::AppState;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

struct TestApp {
    base: PathBuf,
    _dir: Option<tempfile::TempDir>,
    picker: Arc<ScriptedPicker>,
    links: Arc<RecordingOpener>,
    _app: App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
}

impl TestApp {
    fn path(&self, relative: &str) -> PathBuf {
        self.base.join(relative)
    }

    /// What the real app does on exit (RunEvent::Exit): close the library.
    fn shut_down(self) {
        use tauri::Manager;
        let state = self._app.state::<AppState>();
        tauri::async_runtime::block_on(state.session.close());
    }
}

fn dirs(base: &Path) -> AppDirs {
    AppDirs {
        config: base.join("config"),
        logs: base.join("logs"),
        documents: Some(base.join("Dokumente")),
    }
}

/// An app instance whose settings and default library folder live in
/// `base`.
fn app_at(base: &Path, dir: Option<tempfile::TempDir>) -> TestApp {
    let picker = Arc::new(ScriptedPicker::default());
    let links = Arc::new(RecordingOpener::default());
    let state = tauri::async_runtime::block_on(AppState::initialize(
        dirs(base),
        picker.clone(),
        picker.clone(),
        links.clone(),
        Generators::system(),
    ))
    .unwrap();
    let app = mock_builder()
        .manage(state)
        .invoke_handler(commands::handler())
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    TestApp {
        base: base.to_path_buf(),
        _dir: dir,
        picker,
        links,
        _app: app,
        webview,
    }
}

fn app() -> TestApp {
    let dir = common::temp_dir("pw-ipc-");
    let base = dir.path().to_path_buf();
    app_at(&base, Some(dir))
}

/// Origin of the app's own pages: Tauri serves them from `http://tauri.localhost`
/// on Windows and from `tauri://localhost` elsewhere.
fn local_origin() -> &'static str {
    if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    }
}

fn invoke(
    webview: &WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    invoke_from(webview, local_origin(), command, body)
}

/// Calls a command with `{ request }`.
fn call(app: &TestApp, command: &str, request: Value) -> Result<Value, Value> {
    invoke(&app.webview, command, json!({ "request": request }))
}

fn invoke_from(
    webview: &WebviewWindow<MockRuntime>,
    origin: &str,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|response| response.deserialize::<Value>().expect("JSON response"))
}

fn assert_error_code(error: &Value, code: &str) {
    let object = error
        .as_object()
        .unwrap_or_else(|| panic!("error is not an object: {error}"));
    assert!(
        object
            .keys()
            .all(|k| ["code", "messageKey", "details"].contains(&k.as_str())),
        "{error}"
    );
    assert_eq!(error["code"], code, "{error}");
    assert!(error["messageKey"].as_str().unwrap().starts_with("error."));
}

fn assert_uniform_error(error: &Value, field: &str) {
    let object = error
        .as_object()
        .unwrap_or_else(|| panic!("error is not an object: {error}"));
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "details", "messageKey"], "{error}");
    assert_eq!(error["code"], "INVALID_INPUT");
    assert_eq!(error["messageKey"], "error.invalidInput");
    assert_eq!(error["details"]["field"], field, "{error}");
    assert!(error["details"]["reason"].is_string());
}

#[test]
fn valid_request_returns_typed_result() {
    let app = app();
    let value = invoke(
        &app.webview,
        "system_check",
        json!({ "request": { "checks": ["database", "migrations", "foreignKeys", "fullTextSearch"] } }),
    )
    .expect("valid request succeeds");

    assert_eq!(value["appVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["platform"], std::env::consts::OS);
    assert!(value["sqliteVersion"].as_str().unwrap().starts_with("3."));
    assert_eq!(value["schemaVersion"], Database::supported_version());
    let results = value["results"].as_array().unwrap();
    let checks: Vec<&str> = results
        .iter()
        .map(|r| r["check"].as_str().unwrap())
        .collect();
    assert_eq!(
        checks,
        ["database", "migrations", "foreignKeys", "fullTextSearch"]
    );
    assert!(results.iter().all(|r| r["passed"] == true), "{value}");
}

#[test]
fn invalid_requests_return_the_uniform_error_format() {
    let app = app();
    let cases = [
        (json!({}), "request"),
        (json!({ "request": null }), "request"),
        (json!({ "request": "database" }), "request"),
        (json!({ "request": { "checks": "database" } }), "request"),
        (
            json!({ "request": { "checks": ["database", 7] } }),
            "request",
        ),
        (
            json!({ "request": { "checks": ["../../etc/passwd"] } }),
            "request",
        ),
        (
            json!({ "request": { "checks": ["database"], "path": "/etc" } }),
            "request",
        ),
        (json!({ "request": { "checks": [] } }), "checks"),
        (
            json!({ "request": { "checks": ["database", "database"] } }),
            "checks",
        ),
    ];
    for (body, field) in cases {
        let error = invoke(&app.webview, "system_check", body.clone())
            .expect_err(&format!("must fail: {body}"));
        assert_uniform_error(&error, field);
    }
}

#[test]
fn link_open_hands_web_and_mail_addresses_to_the_system() {
    let app = app();
    for url in [
        "https://example.org/seite?x=1#teil",
        "http://example.org",
        "mailto:anna@example.org",
    ] {
        assert_eq!(
            call(&app, "link_open", json!({ "url": url })),
            Ok(Value::Null),
            "{url}"
        );
    }
    assert_eq!(
        app.links.opened(),
        [
            "https://example.org/seite?x=1#teil",
            "http://example.org",
            "mailto:anna@example.org"
        ]
    );
}

#[test]
fn link_open_refuses_other_addresses_and_malformed_requests() {
    let app = app();
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "ftp://example.org",
        "pwrite://dokument",
        "https://example.org/mit leerzeichen",
    ] {
        let error = call(&app, "link_open", json!({ "url": url })).unwrap_err();
        assert_error_code(&error, "LINK_NOT_ALLOWED");
    }
    let too_long = format!("https://example.org/{}", "a".repeat(2048));
    for request in [
        json!({ "url": "" }),
        json!({ "url": too_long }),
        json!({}),
        json!({ "url": 7 }),
        json!({ "url": "https://example.org", "target": "_self" }),
    ] {
        let error = call(&app, "link_open", request.clone()).unwrap_err();
        assert_eq!(error["code"], "INVALID_INPUT", "{request}: {error}");
    }
    assert!(app.links.opened().is_empty());
}

#[test]
fn non_object_payload_is_rejected_in_the_uniform_format() {
    let app = app();
    let error = invoke(&app.webview, "system_check", json!([1, 2, 3])).unwrap_err();
    assert_uniform_error(&error, "request");
}

#[test]
fn unknown_commands_are_rejected_by_tauri() {
    // Tauri itself answers with a plain string here; the frontend wrapper
    // (src/services/ipc.ts) normalises it to IPC_REJECTED.
    let app = app();
    let error = invoke(
        &app.webview,
        "read_text_file",
        json!({ "path": "/etc/passwd" }),
    )
    .unwrap_err();
    assert!(error.is_string(), "{error}");
}

#[test]
fn commands_are_refused_for_remote_origins() {
    // Only the app's own pages may use IPC; there is no "remote" capability.
    let app = app();
    let body = json!({ "request": { "checks": ["database"] } });
    for origin in ["https://example.com", "http://tauri.localhost.example.net"] {
        let error = invoke_from(&app.webview, origin, "system_check", body.clone()).unwrap_err();
        assert!(error.is_string(), "{origin}: {error}");
    }
}

// ---------------------------------------------------------------- library

fn default_location(app: &TestApp) -> Value {
    call(app, "library_default_location", json!({})).unwrap()
}

#[test]
fn first_start_creates_the_library_at_the_default_location() {
    let app = app();
    let status = call(&app, "library_status", json!({})).unwrap();
    assert_eq!(status["state"], "closed", "{status}");
    assert_eq!(status["configuredPath"], Value::Null);

    let choice = default_location(&app);
    assert_eq!(choice["purpose"], "create");
    assert_eq!(choice["contents"], "missing");
    assert_eq!(choice["location"]["kind"], "local");
    let shown = choice["displayPath"].as_str().unwrap();
    assert!(shown.ends_with("ProjectWrite"), "{shown}");

    let status = call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap();
    assert_eq!(status["state"], "open", "{status}");
    let library = &status["library"];
    assert_eq!(library["name"], "ProjectWrite");
    assert_eq!(library["journalMode"], "wal");
    assert_eq!(library["documentCount"], 0);
    assert_eq!(library["locationAccepted"], true);
    assert_eq!(library["schemaVersion"], Database::supported_version());
    assert!(app.path("Dokumente/ProjectWrite/library.json").is_file());

    // The path is remembered in the app settings.
    let settings: Value =
        serde_json::from_slice(&std::fs::read(app.path("config/settings.json")).unwrap()).unwrap();
    assert!(
        settings["libraryPath"]
            .as_str()
            .unwrap()
            .ends_with("ProjectWrite")
    );

    // The same location again: it is a library now.
    let again = default_location(&app);
    assert_eq!(again["contents"], "library");
    let error = call(
        &app,
        "library_create",
        json!({ "location": again["token"], "acceptWarning": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "FOLDER_IS_LIBRARY");
}

#[test]
fn the_next_start_opens_the_remembered_library_and_a_second_instance_is_refused() {
    let dir = common::temp_dir("pw-ipc-");
    let first = app_at(dir.path(), None);
    let choice = default_location(&first);
    call(
        &first,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap();

    // A second instance with the same settings while the first holds the
    // library: it starts without a library and names the reason.
    let second = app_at(dir.path(), None);
    let status = call(&second, "library_status", json!({})).unwrap();
    assert_eq!(status["state"], "closed", "{status}");
    assert_eq!(status["error"]["code"], "LIBRARY_LOCKED", "{status}");
    assert!(
        status["configuredPath"]
            .as_str()
            .unwrap()
            .ends_with("ProjectWrite")
    );
    let error = call(&second, "library_reopen", json!({})).unwrap_err();
    assert_error_code(&error, "LIBRARY_LOCKED");

    // Once the first instance has ended, "try again" opens it.
    first.shut_down();
    let status = call(&second, "library_reopen", json!({})).unwrap();
    assert_eq!(status["state"], "open", "{status}");
    second.shut_down();

    // A fresh start opens the remembered library by itself.
    let third = app_at(dir.path(), None);
    let status = call(&third, "library_status", json!({})).unwrap();
    assert_eq!(status["state"], "open", "{status}");
}

#[test]
fn folders_come_from_the_dialog_and_are_referred_to_by_token() {
    let app = app();
    let chosen = app.path("Auswahl");
    std::fs::create_dir_all(&chosen).unwrap();
    std::fs::write(chosen.join("Brief.txt"), b"x").unwrap();
    app.picker.push(Some(chosen.clone()));
    app.picker.push(None);

    let choice = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "create", "title": "Ort wählen" }),
    )
    .unwrap();
    // A folder with other files gets the library as subfolder.
    assert_eq!(choice["contents"], "missing");
    assert!(
        choice["displayPath"]
            .as_str()
            .unwrap()
            .ends_with("ProjectWrite")
    );
    assert_eq!(app.picker.requests()[0].title, "Ort wählen");

    // Cancelled dialog.
    let cancelled = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "open", "title": "Bibliothek öffnen" }),
    )
    .unwrap();
    assert_eq!(cancelled, Value::Null);

    // Writing commands need a token chosen for them.
    app.picker.push(Some(app.path("Ziel")));
    let open_choice = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "open", "title": "Bibliothek öffnen" }),
    )
    .unwrap();
    let error = call(
        &app,
        "library_create",
        json!({ "location": open_choice["token"], "acceptWarning": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "LOCATION_EXPIRED");
    assert!(!app.path("Ziel").exists());
    call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap();
    assert!(chosen.join("ProjectWrite/library.json").is_file());
    assert_eq!(std::fs::read(chosen.join("Brief.txt")).unwrap(), b"x");

    // Opening accepts any token: the create token now points at a library.
    let status = call(
        &app,
        "library_open",
        json!({ "location": choice["token"], "acceptMoved": false }),
    )
    .unwrap();
    assert_eq!(status["state"], "open");
}

#[test]
fn sync_folders_need_a_confirmation_and_use_the_rollback_journal() {
    let app = app();
    let dropbox = app.path("Dropbox");
    std::fs::create_dir_all(&dropbox).unwrap();
    app.picker.push(Some(dropbox.clone()));
    let choice = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "create", "title": "Ort wählen" }),
    )
    .unwrap();
    assert_eq!(choice["location"]["kind"], "syncFolder", "{choice}");
    assert_eq!(choice["location"]["provider"], "Dropbox");

    let error = call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "LOCATION_NOT_ACCEPTED");
    assert!(!dropbox.join("library.json").exists());

    let status = call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": true }),
    )
    .unwrap();
    assert_eq!(status["library"]["journalMode"], "rollback", "{status}");
    assert_eq!(status["library"]["locationAccepted"], true);
}

#[test]
fn library_commands_reject_invalid_input_uniformly() {
    let app = app();
    let token = default_location(&app)["token"].clone();
    let cases: Vec<(&str, Value, &str)> = vec![
        ("library_status", json!({ "x": 1 }), "request"),
        ("library_default_location", json!(null), "request"),
        (
            "library_choose_location",
            json!({ "purpose": "delete", "title": "x" }),
            "request",
        ),
        (
            "library_choose_location",
            json!({ "purpose": "open" }),
            "request",
        ),
        (
            "library_choose_location",
            json!({ "purpose": "open", "title": "" }),
            "title",
        ),
        (
            "library_choose_location",
            json!({ "purpose": "open", "title": "a\nb" }),
            "title",
        ),
        ("library_create", json!({ "location": token }), "request"),
        (
            "library_create",
            json!({ "location": "../../etc", "acceptWarning": true }),
            "location",
        ),
        (
            "library_create",
            json!({ "location": "/home/a/ProjectWrite", "acceptWarning": true }),
            "location",
        ),
        (
            "library_create",
            json!({ "location": token, "acceptWarning": true, "path": "/tmp" }),
            "request",
        ),
        (
            "library_open",
            json!({ "location": 5, "acceptMoved": false }),
            "request",
        ),
        (
            "library_open",
            json!({ "location": "C:\\Windows", "acceptMoved": false }),
            "location",
        ),
        (
            "library_move",
            json!({ "location": "", "acceptWarning": false }),
            "location",
        ),
        ("library_cancel_move", json!([]), "request"),
        ("library_accept_location", json!("now"), "request"),
    ];
    for (command, request, field) in cases {
        let mut body = json!({ "request": request });
        if command == "library_move" {
            body["onProgress"] = json!("__CHANNEL__:1");
        }
        let error = invoke(&app.webview, command, body)
            .expect_err(&format!("{command} must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    // Well-formed but unknown token.
    let error = call(
        &app,
        "library_open",
        json!({ "location": "0123456789abcdef0123456789abcdef", "acceptMoved": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "LOCATION_EXPIRED");
}

#[test]
fn commands_that_need_a_library_say_so() {
    let app = app();
    let error = call(&app, "library_accept_location", json!({})).unwrap_err();
    assert_error_code(&error, "LIBRARY_NOT_OPEN");
    assert_eq!(call(&app, "library_cancel_move", json!({})).unwrap(), false);
    let status = call(&app, "library_reopen", json!({})).unwrap();
    assert_eq!(status["state"], "closed");
}

#[test]
fn opening_a_folder_without_a_library_fails_cleanly() {
    let app = app();
    let empty = app.path("leer");
    std::fs::create_dir_all(&empty).unwrap();
    app.picker.push(Some(empty.clone()));
    let choice = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "open", "title": "Bibliothek öffnen" }),
    )
    .unwrap();
    assert_eq!(choice["contents"], "empty");
    let error = call(
        &app,
        "library_open",
        json!({ "location": choice["token"], "acceptMoved": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "LIBRARY_NOT_FOUND");
    assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
}

#[test]
fn the_library_moves_through_the_command_with_progress() {
    let app = app();
    let choice = default_location(&app);
    call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap();
    let target = app.path("Neu");
    app.picker.push(Some(target.clone()));
    let choice = call(
        &app,
        "library_choose_location",
        json!({ "purpose": "move", "title": "Neuer Ort" }),
    )
    .unwrap();
    let status = invoke(
        &app.webview,
        "library_move",
        json!({
            "request": { "location": choice["token"], "acceptWarning": false },
            "onProgress": "__CHANNEL__:7"
        }),
    )
    .unwrap();
    assert_eq!(status["state"], "open", "{status}");
    assert!(
        status["library"]["displayPath"]
            .as_str()
            .unwrap()
            .ends_with("Neu")
    );
    assert!(target.join("library.json").is_file());
    // The old folder stays.
    assert!(app.path("Dokumente/ProjectWrite/library.json").is_file());
}

// ---------------------------------------------------------------------------
// Interface settings (Phase 3)

fn default_ui_settings() -> Value {
    json!({
        "language": "system",
        "theme": "system",
        "zoom": 1.0,
        "sidebar": { "visible": true, "width": 260 },
        "limitLineWidth": true,
        "readingTime": { "visible": false, "wordsPerMinute": 200 },
        "autosaveExternal": false,
        "keybindings": {}
    })
}

#[test]
fn ui_state_returns_the_settings_language_and_platform() {
    let app = app();
    let state = call(&app, "ui_state", json!({})).unwrap();
    assert_eq!(state["settings"], default_ui_settings());
    assert!(["de", "en"].contains(&state["locale"].as_str().unwrap()));
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    assert_eq!(state["platform"], platform);
    assert_eq!(state["readOnly"], false);
    assert!(
        !app.path("config/settings.json").exists(),
        "reading must not write"
    );
    app.shut_down();
}

#[test]
fn ui_settings_are_saved_and_used_after_a_restart() {
    let dir = common::temp_dir("pw-ipc-");
    let base = dir.path().to_path_buf();
    let app = app_at(&base, None);
    let mut settings = default_ui_settings();
    settings["language"] = json!("de");
    settings["theme"] = json!("dark");
    settings["zoom"] = json!(1.25);
    settings["sidebar"] = json!({ "visible": false, "width": 320 });
    settings["limitLineWidth"] = json!(false);
    settings["readingTime"] = json!({ "visible": true, "wordsPerMinute": 250 });
    settings["keybindings"] = json!({ "palette.show": ["Ctrl+K"], "focus.toggle": [] });
    let state = call(
        &app,
        "ui_update_settings",
        json!({ "settings": settings.clone() }),
    )
    .unwrap();
    assert_eq!(state["settings"], settings);
    assert_eq!(state["locale"], "de", "the setting overrides the system");

    let file: Value =
        serde_json::from_slice(&std::fs::read(app.path("config/settings.json")).unwrap()).unwrap();
    assert_eq!(file["zoom"], 1.25);
    assert_eq!(file["keybindings"]["palette.show"], json!(["Ctrl+K"]));
    app.shut_down();

    let again = app_at(&base, Some(dir));
    let state = call(&again, "ui_state", json!({})).unwrap();
    assert_eq!(state["settings"], settings);
    let mut english = settings.clone();
    english["language"] = json!("en");
    let state = call(&again, "ui_update_settings", json!({ "settings": english })).unwrap();
    assert_eq!(state["locale"], "en");
    again.shut_down();
}

#[test]
fn ui_update_rejects_invalid_settings_uniformly() {
    let app = app();
    let with = |key: &str, value: Value| {
        let mut settings = default_ui_settings();
        settings[key] = value;
        json!({ "settings": settings })
    };
    let mut missing = default_ui_settings();
    missing.as_object_mut().unwrap().remove("zoom");
    let mut unknown = default_ui_settings();
    unknown["fontUrl"] = json!("https://example.com/font.woff");
    let cases = [
        (json!({}), "request"),
        (json!({ "settings": missing }), "request"),
        (json!({ "settings": unknown }), "request"),
        (with("language", json!("fr")), "request"),
        (with("zoom", json!("big")), "request"),
        (with("sidebar", json!({ "visible": true })), "request"),
        (with("zoom", json!(3.0)), "zoom"),
        (with("zoom", json!(0.1)), "zoom"),
        (with("theme", json!("neon")), "theme"),
        (
            with(
                "readingTime",
                json!({ "visible": true, "wordsPerMinute": 5 }),
            ),
            "readingTime",
        ),
        (with("theme", json!("custom:../../evil")), "theme"),
        (
            with("sidebar", json!({ "visible": true, "width": 20 })),
            "sidebar",
        ),
        (
            with("keybindings", json!({ "palette.show": ["P"] })),
            "keybindings",
        ),
        (
            with(
                "keybindings",
                json!({ "palette.show": ["Ctrl+K", "Ctrl+K"] }),
            ),
            "keybindings",
        ),
        (
            with("keybindings", json!({ "../evil": ["Ctrl+K"] })),
            "keybindings",
        ),
    ];
    for (request, field) in cases {
        let error = call(&app, "ui_update_settings", request.clone())
            .expect_err(&format!("must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    assert!(
        !app.path("config/settings.json").exists(),
        "nothing invalid was written"
    );
    app.shut_down();
}

#[test]
fn ui_settings_of_a_newer_version_are_used_but_not_changed() {
    let dir = common::temp_dir("pw-ipc-");
    let base = dir.path().to_path_buf();
    std::fs::create_dir_all(base.join("config")).unwrap();
    let content = r#"{"formatVersion": 9, "language": "de", "zoom": 1.5}"#;
    std::fs::write(base.join("config/settings.json"), content).unwrap();
    let app = app_at(&base, Some(dir));
    let state = call(&app, "ui_state", json!({})).unwrap();
    assert_eq!(state["readOnly"], true);
    assert_eq!(state["locale"], "de");
    assert_eq!(state["settings"]["zoom"], 1.5);
    let error = call(
        &app,
        "ui_update_settings",
        json!({ "settings": default_ui_settings() }),
    )
    .unwrap_err();
    assert_error_code(&error, "SETTINGS_READ_ONLY");
    assert_eq!(
        std::fs::read_to_string(base.join("config/settings.json")).unwrap(),
        content
    );
    app.shut_down();
}

#[test]
fn ui_event_menu_focus_and_close_commands_validate_their_input() {
    let app = app();
    // Valid requests.
    invoke(
        &app.webview,
        "ui_subscribe",
        json!({ "request": {}, "onEvent": "__CHANNEL__:3" }),
    )
    .unwrap();
    call(
        &app,
        "ui_menu_state",
        json!({ "items": {
            "document.new": { "enabled": true, "shortcut": "Strg+N" },
            "palette.show": { "enabled": true, "shortcut": null },
            "tab.close": { "enabled": false, "shortcut": "Strg+W" }
        } }),
    )
    .unwrap();
    call(&app, "ui_set_focus_mode", json!({ "enabled": true })).unwrap();
    call(&app, "ui_set_focus_mode", json!({ "enabled": false })).unwrap();
    call(&app, "ui_close_response", json!({ "close": false })).unwrap();

    // Invalid requests.
    let cases = [
        ("ui_menu_state", json!({}), "request"),
        (
            "ui_menu_state",
            json!({ "items": { "document.delete": { "enabled": true, "shortcut": null } } }),
            "request",
        ),
        (
            "ui_menu_state",
            json!({ "items": { "document.new": { "enabled": "yes", "shortcut": null } } }),
            "request",
        ),
        (
            "ui_menu_state",
            json!({ "items": { "document.new": { "enabled": true, "shortcut": "x".repeat(33) } } }),
            "shortcut",
        ),
        (
            "ui_menu_state",
            json!({ "items": { "document.new": { "enabled": true, "shortcut": "" } } }),
            "shortcut",
        ),
        (
            "ui_menu_state",
            json!({ "items": { "document.new": { "enabled": true, "shortcut": "Strg\nN" } } }),
            "shortcut",
        ),
        ("ui_set_focus_mode", json!({}), "request"),
        ("ui_set_focus_mode", json!({ "enabled": "yes" }), "request"),
        ("ui_close_response", json!({}), "request"),
        (
            "ui_close_response",
            json!({ "close": true, "force": true }),
            "request",
        ),
    ];
    for (command, request, field) in cases {
        let error = call(&app, command, request.clone())
            .expect_err(&format!("{command} must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    let error =
        invoke(&app.webview, "ui_subscribe", json!({ "request": [] })).expect_err("array request");
    assert_uniform_error(&error, "request");
    app.shut_down();
}

// ---------------------------------------------------------------------------
// Documents (Phase 3)

/// An app with a new library at the default location.
fn app_with_library() -> TestApp {
    let app = app();
    let choice = default_location(&app);
    let status = call(
        &app,
        "library_create",
        json!({ "location": choice["token"], "acceptWarning": false }),
    )
    .unwrap();
    assert_eq!(status["state"], "open", "{status}");
    app
}

fn base64(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[test]
fn documents_are_created_listed_read_saved_and_renamed() {
    let app = app_with_library();
    let created = call(
        &app,
        "document_create",
        json!({ "title": "  Notiz  ", "format": "txt" }),
    )
    .unwrap();
    assert_eq!(created["title"], "Notiz");
    assert_eq!(created["format"], "txt");
    assert_eq!(created["sizeBytes"], 0);
    assert_eq!(created["originalName"], Value::Null);
    let id = created["id"].as_str().unwrap().to_owned();

    let list = call(&app, "documents_list", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], id.as_str());

    let read = call(&app, "document_read", json!({ "id": id })).unwrap();
    assert_eq!(read["text"], "");

    let text = "Größe, Maß und „Anführungszeichen“\n\tmit Tab";
    let saved = call(&app, "document_save", json!({ "id": id, "text": text })).unwrap();
    assert_eq!(saved["sizeBytes"], text.len());
    let read = call(&app, "document_read", json!({ "id": id })).unwrap();
    assert_eq!(read["text"], text);
    // Unchanged text is not written again.
    let again = call(&app, "document_save", json!({ "id": id, "text": text })).unwrap();
    assert_eq!(again["updatedAt"], saved["updatedAt"]);

    let renamed = call(
        &app,
        "document_rename",
        json!({ "id": id, "title": "Kapitel 1" }),
    )
    .unwrap();
    assert_eq!(renamed["title"], "Kapitel 1");
    assert_eq!(
        call(&app, "document_read", json!({ "id": id })).unwrap()["text"],
        text,
        "renaming changes only the title"
    );

    let markdown = call(
        &app,
        "document_create",
        json!({ "title": "Plan", "format": "md" }),
    )
    .unwrap();
    assert_eq!(markdown["format"], "md");
    assert_eq!(
        call(&app, "documents_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    app.shut_down();
}

#[test]
fn documents_are_copied_marked_as_favorites_and_recently_opened() {
    let app = app_with_library();
    let created = call(
        &app,
        "document_create",
        json!({ "title": "Kapitel", "format": "md" }),
    )
    .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["favoritedAt"], Value::Null);
    assert_eq!(created["lastOpenedAt"], Value::Null);
    call(
        &app,
        "document_save",
        json!({ "id": id, "text": "# Kapitel\n" }),
    )
    .unwrap();

    // Opening is what "Zuletzt verwendet" lists; it changes no content.
    let read = call(&app, "document_read", json!({ "id": id })).unwrap();
    assert!(read["document"]["lastOpenedAt"].is_string(), "{read}");

    let favorite = call(
        &app,
        "document_set_favorite",
        json!({ "id": id, "favorite": true }),
    )
    .unwrap();
    assert!(favorite["favoritedAt"].is_string());
    assert_eq!(favorite["updatedAt"], read["document"]["updatedAt"]);
    let again = call(
        &app,
        "document_set_favorite",
        json!({ "id": id, "favorite": true }),
    )
    .unwrap();
    assert_eq!(
        again["favoritedAt"], favorite["favoritedAt"],
        "stays a favourite since then"
    );
    let plain = call(
        &app,
        "document_set_favorite",
        json!({ "id": id, "favorite": false }),
    )
    .unwrap();
    assert_eq!(plain["favoritedAt"], Value::Null);

    let copy = call(
        &app,
        "document_copy",
        json!({ "id": id, "title": "Kapitel (Kopie)" }),
    )
    .unwrap();
    assert_ne!(copy["id"], id.as_str());
    assert_eq!(copy["title"], "Kapitel (Kopie)");
    assert_eq!(copy["format"], "md");
    let copied = call(&app, "document_read", json!({ "id": copy["id"] })).unwrap();
    assert_eq!(copied["text"], "# Kapitel\n");
    // The copy is a document of its own.
    call(
        &app,
        "document_save",
        json!({ "id": copy["id"], "text": "anders\n" }),
    )
    .unwrap();
    assert_eq!(
        call(&app, "document_read", json!({ "id": id })).unwrap()["text"],
        "# Kapitel\n"
    );
    let missing = call(
        &app,
        "document_copy",
        json!({ "id": "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c", "title": "X" }),
    )
    .unwrap_err();
    assert_error_code(&missing, "DOCUMENT_NOT_FOUND");
    app.shut_down();
}

#[test]
fn pwrite_documents_keep_their_envelope_and_unknown_fields() {
    let app = app_with_library();
    let created = call(
        &app,
        "document_create",
        json!({ "title": "Roman", "format": "pwrite" }),
    )
    .unwrap();
    assert_eq!(created["format"], "pwrite");
    let id = created["id"].as_str().unwrap().to_owned();
    let file = app.path(&format!(
        "Dokumente/ProjectWrite/documents/richtext/{id}.pwrite"
    ));
    let envelope: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(envelope["format"], "pwrite");
    assert_eq!(envelope["formatVersion"], 1);
    assert_eq!(envelope["id"], id.as_str());
    assert_eq!(envelope["title"], "Roman");

    let read = call(&app, "document_read", json!({ "id": id })).unwrap();
    assert_eq!(read["readOnly"], false);
    let content: Value = serde_json::from_str(read["text"].as_str().unwrap()).unwrap();
    assert_eq!(content["type"], "doc");

    // A field of a later version survives saving.
    let mut stored = envelope.clone();
    stored["future"] = json!({ "keep": true });
    std::fs::write(&file, serde_json::to_vec(&stored).unwrap()).unwrap();
    let text = json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hallo" }] }] });
    let saved = call(
        &app,
        "document_save",
        json!({ "id": id, "text": text.to_string() }),
    )
    .unwrap();
    let after: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(after["content"], text);
    assert_eq!(after["future"], json!({ "keep": true }));
    // Unchanged content is not written again.
    let bytes = std::fs::read(&file).unwrap();
    let again = call(
        &app,
        "document_save",
        json!({ "id": id, "text": text.to_string() }),
    )
    .unwrap();
    assert_eq!(again["updatedAt"], saved["updatedAt"]);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);

    // The copy has its own envelope.
    let copy = call(
        &app,
        "document_copy",
        json!({ "id": id, "title": "Roman (Kopie)" }),
    )
    .unwrap();
    let copy_id = copy["id"].as_str().unwrap();
    let copied: Value = serde_json::from_slice(
        &std::fs::read(app.path(&format!(
            "Dokumente/ProjectWrite/documents/richtext/{copy_id}.pwrite"
        )))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(copied["id"], copy_id);
    assert_eq!(copied["title"], "Roman (Kopie)");
    assert_eq!(copied["content"], text);

    for bad in ["kein JSON", "[1]", r#"{"type": "paragraph"}"#] {
        let error = call(&app, "document_save", json!({ "id": id, "text": bad })).unwrap_err();
        assert!(
            error["code"] == "INVALID_INPUT" || error["code"] == "PWRITE_INVALID",
            "{bad}: {error}"
        );
    }

    // A newer format version opens read only and is never written.
    let mut newer = after.clone();
    newer["formatVersion"] = json!(2);
    std::fs::write(&file, serde_json::to_vec(&newer).unwrap()).unwrap();
    let read = call(&app, "document_read", json!({ "id": id })).unwrap();
    assert_eq!(read["readOnly"], true);
    let before = std::fs::read(&file).unwrap();
    let error = call(
        &app,
        "document_save",
        json!({ "id": id, "text": json!({ "type": "doc", "content": [] }).to_string() }),
    )
    .unwrap_err();
    assert_error_code(&error, "DOCUMENT_READ_ONLY");
    assert_eq!(std::fs::read(&file).unwrap(), before);

    std::fs::write(&file, b"{\"format\": \"other\"}").unwrap();
    let error = call(&app, "document_read", json!({ "id": id })).unwrap_err();
    assert_error_code(&error, "PWRITE_INVALID");
    app.shut_down();
}

#[test]
fn document_commands_need_an_open_library() {
    let app = app();
    let id = "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c";
    for (command, request) in [
        ("documents_list", json!({})),
        ("document_create", json!({ "title": "A", "format": "txt" })),
        ("document_read", json!({ "id": id })),
        ("document_save", json!({ "id": id, "text": "" })),
        ("document_rename", json!({ "id": id, "title": "B" })),
        ("document_copy", json!({ "id": id, "title": "B" })),
        (
            "document_set_favorite",
            json!({ "id": id, "favorite": true }),
        ),
        (
            "document_import",
            json!({ "fileName": "a.txt", "contentBase64": "", "allowDuplicate": false }),
        ),
    ] {
        let error = call(&app, command, request).unwrap_err();
        assert_error_code(&error, "LIBRARY_NOT_OPEN");
    }
    app.shut_down();
}

#[test]
fn document_commands_reject_invalid_input_uniformly() {
    let app = app_with_library();
    let id = call(
        &app,
        "document_create",
        json!({ "title": "A", "format": "txt" }),
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let cases = [
        ("documents_list", json!({ "all": true }), "request"),
        (
            "document_create",
            json!({ "title": "", "format": "txt" }),
            "title",
        ),
        (
            "document_create",
            json!({ "title": "a\nb", "format": "txt" }),
            "title",
        ),
        (
            "document_create",
            json!({ "title": "x".repeat(301), "format": "md" }),
            "title",
        ),
        (
            "document_create",
            json!({ "title": "A", "format": "docx" }),
            "request",
        ),
        ("document_create", json!({ "title": "A" }), "request"),
        ("document_read", json!({ "id": "../../library.json" }), "id"),
        ("document_read", json!({ "id": id.to_uppercase() }), "id"),
        (
            "document_read",
            json!({ "id": id, "path": "/etc" }),
            "request",
        ),
        ("document_save", json!({ "id": id }), "request"),
        ("document_save", json!({ "id": "x", "text": "" }), "id"),
        (
            "document_rename",
            json!({ "id": id, "title": " " }),
            "title",
        ),
        ("document_copy", json!({ "id": id, "title": "" }), "title"),
        ("document_copy", json!({ "id": "x", "title": "A" }), "id"),
        ("document_copy", json!({ "id": id }), "request"),
        (
            "document_set_favorite",
            json!({ "id": id, "favorite": "yes" }),
            "request",
        ),
        ("document_set_favorite", json!({ "id": id }), "request"),
        (
            "document_set_favorite",
            json!({ "id": "../x", "favorite": true }),
            "id",
        ),
        (
            "document_import",
            json!({ "fileName": "../a.txt", "contentBase64": "", "allowDuplicate": false }),
            "fileName",
        ),
        (
            "document_import",
            json!({ "fileName": "a.txt", "contentBase64": "%%%", "allowDuplicate": false }),
            "contentBase64",
        ),
        (
            "document_import",
            json!({ "fileName": "a.txt", "contentBase64": "" }),
            "request",
        ),
    ];
    for (command, request, field) in cases {
        let error = call(&app, command, request.clone())
            .expect_err(&format!("{command} must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    let unknown = "1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c";
    for (command, request) in [
        ("document_read", json!({ "id": unknown })),
        ("document_save", json!({ "id": unknown, "text": "x" })),
        ("document_rename", json!({ "id": unknown, "title": "x" })),
    ] {
        assert_error_code(
            &call(&app, command, request).unwrap_err(),
            "DOCUMENT_NOT_FOUND",
        );
    }
    app.shut_down();
}

#[test]
fn dropped_text_files_are_imported_with_a_duplicate_question() {
    let app = app_with_library();
    let content = "\u{FEFF}# Notiz\r\nZeile zwei\r\n".as_bytes();
    let first = call(
        &app,
        "document_import",
        json!({ "fileName": "Notiz.md", "contentBase64": base64(content), "allowDuplicate": false }),
    )
    .unwrap();
    assert_eq!(first["kind"], "imported", "{first}");
    let document = &first["document"];
    assert_eq!(document["title"], "Notiz");
    assert_eq!(document["format"], "md");
    assert_eq!(document["originalName"], "Notiz.md");
    let text = call(&app, "document_read", json!({ "id": document["id"] })).unwrap();
    assert_eq!(
        text["text"], "# Notiz\nZeile zwei\n",
        "BOM removed, LF line ends"
    );

    // The same text again (also from another file name): ask first.
    let duplicate = call(
        &app,
        "document_import",
        json!({ "fileName": "Kopie.md", "contentBase64": base64("# Notiz\nZeile zwei\n".as_bytes()), "allowDuplicate": false }),
    )
    .unwrap();
    assert_eq!(duplicate["kind"], "duplicate", "{duplicate}");
    assert_eq!(duplicate["existing"]["id"], document["id"]);
    let anyway = call(
        &app,
        "document_import",
        json!({ "fileName": "Kopie.md", "contentBase64": base64(content), "allowDuplicate": true }),
    )
    .unwrap();
    assert_eq!(anyway["kind"], "imported");
    assert_ne!(anyway["document"]["id"], document["id"]);

    // Type checks: extension positive list and content.
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    assert_error_code(
        &call(
            &app,
            "document_import",
            json!({ "fileName": "bild.png", "contentBase64": base64(png), "allowDuplicate": false }),
        )
        .unwrap_err(),
        "IMPORT_UNSUPPORTED_TYPE",
    );
    assert_error_code(
        &call(
            &app,
            "document_import",
            json!({ "fileName": "getarnt.txt", "contentBase64": base64(png), "allowDuplicate": false }),
        )
        .unwrap_err(),
        "DOCUMENT_NOT_TEXT",
    );
    assert_eq!(
        call(&app, "documents_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2,
        "rejected files leave nothing behind"
    );
    app.shut_down();
}

// ---------------------------------------------------------------------------
// Themes (Phase 3)

fn theme_file(text_muted: &str) -> String {
    json!({
        "formatVersion": 1,
        "name": "Nacht",
        "base": "dark",
        "colors": {
            "surface": "#1E1E1E", "sidebar": "#252526", "text": "#D4D4D4",
            "textMuted": text_muted, "accent": "#6C9BFF", "error": "#F48771",
            "success": "#4EC97A", "clusterBlue": "#6C9BFF", "clusterGreen": "#4EC97A",
            "clusterViolet": "#B58CFF", "clusterOrange": "#F0A050", "clusterGrey": "#9AA0A6"
        }
    })
    .to_string()
}

#[test]
fn themes_are_imported_activated_and_removed() {
    let app = app();
    let state = call(
        &app,
        "theme_import",
        json!({ "content": theme_file("#A0A4AA") }),
    )
    .unwrap();
    let themes = state["customThemes"].as_array().unwrap();
    assert_eq!(themes.len(), 1);
    let id = themes[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(themes[0]["name"], "Nacht");
    assert_eq!(themes[0]["colors"]["textMuted"], "#A0A4AA");
    assert_eq!(state["settings"]["theme"], format!("custom:{id}"));
    assert!(app.path(&format!("config/themes/{id}.json")).is_file());

    let state = call(&app, "theme_remove", json!({ "id": id })).unwrap();
    assert_eq!(state["settings"]["theme"], "system");
    assert!(state["customThemes"].as_array().unwrap().is_empty());
    assert!(!app.path(&format!("config/themes/{id}.json")).exists());
    app.shut_down();
}

#[test]
fn theme_commands_reject_invalid_files_and_input() {
    let app = app();
    let error = call(&app, "theme_import", json!({ "content": "{" })).unwrap_err();
    assert_error_code(&error, "THEME_INVALID");
    let error = call(
        &app,
        "theme_import",
        json!({ "content": theme_file("#555555") }),
    )
    .unwrap_err();
    assert_error_code(&error, "THEME_CONTRAST");
    assert_eq!(error["details"]["pair"], "textMuted/surface");
    let error = call(
        &app,
        "theme_import",
        json!({ "content": theme_file("rgb(1,2,3)") }),
    )
    .unwrap_err();
    assert_error_code(&error, "THEME_INVALID");
    assert_eq!(error["details"]["field"], "colors.textMuted");

    for (command, request, field) in [
        ("theme_import", json!({}), "request"),
        (
            "theme_import",
            json!({ "content": " ".repeat(64 * 1024 + 1) }),
            "content",
        ),
        (
            "theme_import",
            json!({ "content": "{}", "path": "/x" }),
            "request",
        ),
        ("theme_remove", json!({ "id": "../settings" }), "id"),
        (
            "theme_remove",
            json!({ "id": "ABCDEF0123456789ABCDEF0123456789" }),
            "id",
        ),
    ] {
        let error = call(&app, command, request.clone())
            .expect_err(&format!("{command} must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    let error = call(
        &app,
        "theme_remove",
        json!({ "id": "0123456789abcdef0123456789abcdef" }),
    )
    .unwrap_err();
    assert_error_code(&error, "THEME_NOT_FOUND");
    assert!(!app.path("config/themes").exists(), "nothing was stored");
    app.shut_down();
}

/// A file of the user outside the library, inside the test folder.
fn user_file(app: &TestApp, name: &str, data: &[u8]) -> PathBuf {
    let path = app.path(&format!("Eigene Dateien/{name}"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, data).unwrap();
    path
}

fn open_external(app: &TestApp, path: &Path) -> Value {
    app.picker.push(Some(path.to_path_buf()));
    let info = call(app, "external_open", json!({ "title": "Öffnen" })).unwrap();
    assert_eq!(info["name"], path.file_name().unwrap().to_str().unwrap());
    info
}

#[test]
fn external_files_keep_encoding_line_endings_and_unchanged_bytes() {
    let app = app();
    // UTF-16 LE with byte order mark and CRLF.
    let original: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(
            "Größe\r\nZeile\r\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    let path = user_file(&app, "brief.txt", &original);
    let info = open_external(&app, &path);
    assert_eq!(info["format"], "txt");
    assert_eq!(info["large"], false);
    assert!(info["folder"].as_str().unwrap().ends_with("Eigene Dateien"));
    assert!(info.get("path").is_none());
    let id = info["id"].as_str().unwrap().to_owned();
    let (dialog, request) = app.picker.file_requests().pop().unwrap();
    assert_eq!(
        dialog,
        project_write_lib::services::dialog::FileDialog::Open
    );
    assert_eq!(request.title, "Öffnen");

    let text = call(&app, "external_read", json!({ "id": id })).unwrap();
    assert_eq!(text["text"], "Größe\nZeile\n");
    assert_eq!(text["encoding"], "utf16Le");
    assert_eq!(text["lineEnding"], "crlf");
    assert_eq!(text["mixedLineEndings"], false);
    assert_eq!(text["readOnly"], false);

    let save = |text: &str, overwrite: bool| {
        call(
            &app,
            "external_save",
            json!({ "id": id, "text": text, "encoding": "utf16Le", "lineEnding": "crlf", "overwrite": overwrite }),
        )
    };
    assert_eq!(save("Größe\nZeile\n", false).unwrap(), "unchanged");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(save("Größe\nZeile\nNeu\n", false).unwrap(), "saved");
    let expected: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(
            "Größe\r\nZeile\r\nNeu\r\n"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let check = call(&app, "external_check", json!({ "id": id })).unwrap();
    assert_eq!(check, json!({ "status": "unchanged", "readOnly": false }));

    // A change from outside: reported, never overwritten without consent.
    std::fs::write(&path, b"von au\xC3\x9Fen\n").unwrap();
    let check = call(&app, "external_check", json!({ "id": id })).unwrap();
    assert_eq!(check["status"], "changed");
    assert_error_code(&save("Meine Fassung\n", false).unwrap_err(), "FILE_CHANGED");
    assert_eq!(std::fs::read(&path).unwrap(), "von außen\n".as_bytes());
    call(&app, "external_keep_mine", json!({ "id": id })).unwrap();
    assert_eq!(save("Meine Fassung\n", false).unwrap(), "saved");

    // Windows-1252 cannot hold every character: nothing is written.
    let error = call(
        &app,
        "external_save",
        json!({ "id": id, "text": "Pfeil →\n", "encoding": "windows1252", "lineEnding": "lf", "overwrite": false }),
    )
    .unwrap_err();
    assert_error_code(&error, "FILE_UNENCODABLE");
    assert_eq!(error["details"]["codePoint"], "U+2192");

    // Save as: the id stands for the new file, the old one stays.
    let target = app.path("Eigene Dateien/kopie");
    app.picker.push(Some(target.clone()));
    let copy = call(
        &app,
        "external_save_as",
        json!({ "id": id, "title": "Speichern unter", "text": "Kopie\n", "encoding": "utf8", "lineEnding": "lf" }),
    )
    .unwrap();
    assert_eq!(copy["id"], id.as_str());
    assert_eq!(copy["name"], "kopie.txt");
    assert_eq!(
        std::fs::read(app.path("Eigene Dateien/kopie.txt")).unwrap(),
        b"Kopie\n"
    );
    let (dialog, request) = app.picker.file_requests().pop().unwrap();
    assert_eq!(
        dialog,
        project_write_lib::services::dialog::FileDialog::Save
    );
    assert_eq!(request.file_name.as_deref(), Some("brief.txt"));
    // Cancelled dialogs change nothing.
    let cancelled = call(&app, "external_open", json!({ "title": "Öffnen" })).unwrap();
    assert_eq!(cancelled, Value::Null);

    // Recently used, newest first; a missing entry is marked and removable.
    let recent = call(&app, "external_recent", json!({})).unwrap();
    let names: Vec<_> = recent
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["kopie.txt", "brief.txt"]);
    std::fs::remove_file(&path).unwrap();
    let recent = call(&app, "external_recent", json!({})).unwrap();
    assert_eq!(recent[1]["missing"], true);
    let error = call(
        &app,
        "external_open_recent",
        json!({ "id": recent[1]["id"] }),
    )
    .unwrap_err();
    assert_error_code(&error, "FILE_MISSING");
    let left = call(
        &app,
        "external_forget_recent",
        json!({ "id": recent[1]["id"] }),
    )
    .unwrap();
    assert_eq!(left.as_array().unwrap().len(), 1);
    let again = call(
        &app,
        "external_open_recent",
        json!({ "id": recent[0]["id"] }),
    )
    .unwrap();
    assert_eq!(again["id"], id.as_str());
    let settings: Value =
        serde_json::from_slice(&std::fs::read(app.path("config/settings.json")).unwrap()).unwrap();
    assert_eq!(settings["recentFiles"].as_array().unwrap().len(), 1);

    call(&app, "external_close", json!({ "id": id })).unwrap();
    let error = call(&app, "external_read", json!({ "id": id })).unwrap_err();
    assert_error_code(&error, "FILE_UNKNOWN");
    app.shut_down();
}

#[test]
fn external_files_refuse_binary_content_and_foreign_types() {
    let app = app();
    let binary = user_file(&app, "bild.md", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    let info = open_external(&app, &binary);
    let error = call(&app, "external_read", json!({ "id": info["id"] })).unwrap_err();
    assert_error_code(&error, "DOCUMENT_NOT_TEXT");
    let csv = user_file(&app, "tabelle.csv", b"a;b\n");
    app.picker.push(Some(csv));
    let error = call(&app, "external_open", json!({ "title": "Öffnen" })).unwrap_err();
    assert_error_code(&error, "IMPORT_UNSUPPORTED_TYPE");
    app.shut_down();
}

#[test]
fn external_files_are_taken_into_the_library_as_a_copy() {
    let app = app_with_library();
    let original = b"# Kapitel\r\n\r\nText von au\xDFen\r\n".to_vec();
    let path = user_file(&app, "Kapitel 1.md", &original);
    let info = open_external(&app, &path);
    let id = info["id"].as_str().unwrap().to_owned();
    let text = call(&app, "external_read", json!({ "id": id })).unwrap();
    assert_eq!(text["encoding"], "windows1252");
    let result = call(
        &app,
        "external_import",
        json!({ "id": id, "text": text["text"], "allowDuplicate": false }),
    )
    .unwrap();
    assert_eq!(result["kind"], "imported");
    let document = &result["document"];
    assert_eq!(document["title"], "Kapitel 1");
    assert_eq!(document["format"], "md");
    let read = call(&app, "document_read", json!({ "id": document["id"] })).unwrap();
    assert_eq!(read["text"], "# Kapitel\n\nText von außen\n");
    // The original stays untouched.
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let again = call(
        &app,
        "external_import",
        json!({ "id": id, "text": text["text"], "allowDuplicate": false }),
    )
    .unwrap();
    assert_eq!(again["kind"], "duplicate");
    app.shut_down();
}

#[test]
fn external_commands_reject_invalid_input_uniformly() {
    let app = app();
    let id = "0123456789abcdef0123456789abcdef";
    let cases = [
        ("external_open", json!({}), "request"),
        ("external_open", json!({ "title": "" }), "title"),
        ("external_open", json!({ "title": "a\u{7}" }), "title"),
        (
            "external_open",
            json!({ "title": "Öffnen", "path": "/etc/passwd" }),
            "request",
        ),
        ("external_read", json!({ "id": "../../etc/passwd" }), "id"),
        ("external_read", json!({ "id": id.to_uppercase() }), "id"),
        ("external_check", json!({ "id": 7 }), "request"),
        ("external_keep_mine", json!({}), "request"),
        (
            "external_save",
            json!({ "id": id, "text": "", "encoding": "latin9", "lineEnding": "lf", "overwrite": false }),
            "request",
        ),
        (
            "external_save",
            json!({ "id": id, "text": "", "encoding": "utf8", "lineEnding": "cr", "overwrite": false }),
            "request",
        ),
        (
            "external_save",
            json!({ "id": id, "text": "", "encoding": "utf8", "lineEnding": "lf" }),
            "request",
        ),
        (
            "external_save_as",
            json!({ "id": id, "title": "", "text": "", "encoding": "utf8", "lineEnding": "lf" }),
            "title",
        ),
        (
            "external_import",
            json!({ "id": id, "text": "" }),
            "request",
        ),
        ("external_recent", json!({ "limit": 3 }), "request"),
        ("external_open_recent", json!({ "id": "" }), "id"),
        ("external_forget_recent", json!({ "id": "/home" }), "id"),
        (
            "external_close",
            json!({ "id": id, "force": true }),
            "request",
        ),
    ];
    for (command, request, field) in cases {
        let error = call(&app, command, request.clone())
            .expect_err(&format!("{command} must fail: {request}"));
        assert_uniform_error(&error, field);
    }
    // Well-formed but unknown ids.
    for command in ["external_read", "external_check", "external_keep_mine"] {
        let error = call(&app, command, json!({ "id": id })).unwrap_err();
        assert_error_code(&error, "FILE_UNKNOWN");
    }
    let error = call(&app, "external_open_recent", json!({ "id": id })).unwrap_err();
    assert_error_code(&error, "FILE_UNKNOWN");
    app.shut_down();
}
