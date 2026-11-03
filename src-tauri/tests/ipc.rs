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
    let state = tauri::async_runtime::block_on(AppState::initialize(
        dirs(base),
        picker.clone(),
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
            with("keybindings", json!({ "palette.show": ["Ctrl+K", "Ctrl+K"] })),
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
