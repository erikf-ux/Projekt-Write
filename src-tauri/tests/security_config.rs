//! Static checks of the security configuration (sections 7, 8 and 20).
//! They fail the build if someone loosens the configuration by accident.

use std::fs;
use std::path::{Path, PathBuf};

use project_write_lib::commands::COMMAND_NAMES;
use project_write_lib::error::ErrorCode;
use serde_json::{Value, json};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn tauri_config() -> Value {
    read_json(&manifest_dir().join("tauri.conf.json"))
}

#[test]
fn csp_is_exactly_the_section_20_policy() {
    let expected = json!({
        "default-src": "'self'",
        "script-src": "'self'",
        "style-src": "'self' 'unsafe-inline'",
        "img-src": "'self' pwmedia: http://pwmedia.localhost",
        "font-src": "'self'",
        "connect-src": "ipc: http://ipc.localhost pwmedia: http://pwmedia.localhost",
        "worker-src": "'self'",
        "object-src": "'none'",
        "base-uri": "'none'",
        "form-action": "'none'",
        "frame-src": "'none'"
    });
    let config = tauri_config();
    let security = &config["app"]["security"];
    assert_eq!(security["csp"], expected, "every CSP change needs an ADR (section 20)");
    let csp_text = security["csp"].to_string();
    assert!(!csp_text.contains("unsafe-eval"));
    assert!(!csp_text.contains("https:"));
    assert!(security.get("devCsp").is_none(), "no separate, looser development CSP");
    assert!(security.get("dangerousDisableAssetCspModification").is_none());
}

#[test]
fn webview_hardening_is_enabled() {
    let config = tauri_config();
    assert_eq!(config["app"]["withGlobalTauri"], false);
    assert_eq!(config["app"]["security"]["freezePrototype"], true);
    assert_eq!(config["app"]["security"]["assetProtocol"]["enable"], false);
    for window in config["app"]["windows"].as_array().unwrap() {
        assert_eq!(window["dragDropEnabled"], false, "section 11: HTML5 drag and drop");
    }
}

#[test]
fn capabilities_grant_only_the_apps_own_commands() {
    let config = tauri_config();
    let enabled: Vec<&str> = config["app"]["security"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(enabled, ["main-window"]);

    let mut expected: Vec<String> = COMMAND_NAMES.iter().map(|name| format!("allow-{}", name.replace('_', "-"))).collect();
    expected.sort();

    let dir = manifest_dir().join("capabilities");
    let mut files = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        files += 1;
        let capability = read_json(&path);
        let mut permissions: Vec<String> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().expect("permissions are plain identifiers, no scopes").to_owned())
            .collect();
        permissions.sort();
        assert_eq!(permissions, expected, "{}", path.display());
        assert!(capability.get("remote").is_none(), "no remote origins may use IPC");
    }
    assert_eq!(files, 1, "exactly one capability file");
}

#[test]
fn every_command_is_in_the_app_manifest() {
    let build_rs = fs::read_to_string(manifest_dir().join("build.rs")).unwrap();
    for name in COMMAND_NAMES {
        assert!(build_rs.contains(&format!("\"{name}\"")), "{name} missing in build.rs app manifest");
    }
}

#[test]
fn forbidden_dependencies_are_absent() {
    let cargo_toml = fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    for forbidden in ["rusqlite", "tauri-plugin-sql", "tauri-plugin-fs", "tauri-plugin-shell", "tauri-plugin-http"] {
        assert!(!cargo_toml.contains(forbidden), "{forbidden} must not be a dependency (sections 7 and 20)");
    }
    let package_json = fs::read_to_string(manifest_dir().join("../package.json")).unwrap();
    assert!(!package_json.contains("@tauri-apps/plugin-"), "the frontend gets no Tauri plugins");
}

#[test]
fn test_plugin_is_optional_and_feature_gated() {
    let cargo_toml = fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    let line = cargo_toml
        .lines()
        .find(|l| l.starts_with("tauri-plugin-wdio-webdriver"))
        .expect("dependency line");
    assert!(line.contains("optional = true"), "{line}");
    let lib_rs = fs::read_to_string(manifest_dir().join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("#[cfg(all(feature = \"e2e\", not(debug_assertions)))]"));
}

#[test]
fn every_error_message_key_is_translated() {
    for language in ["de", "en"] {
        let dictionary = read_json(&manifest_dir().join(format!("../src/i18n/{language}.json")));
        for code in ErrorCode::ALL {
            let key = code.message_key();
            assert!(dictionary.get(key).and_then(Value::as_str).is_some(), "{key} missing in {language}.json");
        }
    }
}
