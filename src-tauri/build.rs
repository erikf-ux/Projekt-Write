fn main() {
    // App manifest (section 20): every app command must be listed here and is
    // denied unless a capability allows it explicitly. Keep this list in sync
    // with `commands::COMMAND_NAMES` (checked by tests/security_config.rs).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["system_check"])),
    )
    .expect("failed to run tauri-build");
}
