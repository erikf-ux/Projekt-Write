fn main() {
    // App manifest (section 20): every app command must be listed here and is
    // denied unless a capability allows it explicitly. Keep this list in sync
    // with `commands::COMMAND_NAMES` (checked by tests/security_config.rs).
    let app_manifest = tauri_build::AppManifest::new().commands(&["system_check"]);

    let mut attributes = tauri_build::Attributes::new().app_manifest(app_manifest);
    if embed_windows_manifest_for_all_targets() {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

/// Tauri needs Common Controls v6 on Windows. `tauri-build` embeds the
/// manifest only into the app binary, so test executables that link Tauri
/// crash with STATUS_ENTRYPOINT_NOT_FOUND. Linking the manifest ourselves
/// covers the app and every test executable. Returns whether it did so.
fn embed_windows_manifest_for_all_targets() -> bool {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return false;
    }
    let manifest =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    true
}
