//! IPC contract tests through Tauri's real command pipeline (mock runtime):
//! a valid request yields the typed result, every invalid request yields the
//! uniform error format `{ code, messageKey, details? }` (section 8).

use project_write_lib::commands;
use project_write_lib::database::Database;
use project_write_lib::state::AppState;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

fn app() -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let database = tauri::async_runtime::block_on(async {
        let database = Database::open_in_memory().await.unwrap();
        database.migrate().await.unwrap();
        database
    });
    let app = mock_builder()
        .manage(AppState { database })
        .invoke_handler(commands::handler())
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    (app, webview)
}

fn invoke(
    webview: &WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|response| response.deserialize::<Value>().expect("JSON response"))
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
    let (_app, webview) = app();
    let value = invoke(
        &webview,
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
    let (_app, webview) = app();
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
        let error = invoke(&webview, "system_check", body.clone())
            .expect_err(&format!("must fail: {body}"));
        assert_uniform_error(&error, field);
    }
}

#[test]
fn non_object_payload_is_rejected_in_the_uniform_format() {
    let (_app, webview) = app();
    let error = invoke(&webview, "system_check", json!([1, 2, 3])).unwrap_err();
    assert_uniform_error(&error, "request");
}

#[test]
fn unknown_commands_are_rejected_by_tauri() {
    // Tauri itself answers with a plain string here; the frontend wrapper
    // (src/services/ipc.ts) normalises it to IPC_REJECTED.
    let (_app, webview) = app();
    let error = invoke(&webview, "read_text_file", json!({ "path": "/etc/passwd" })).unwrap_err();
    assert!(error.is_string(), "{error}");
}
