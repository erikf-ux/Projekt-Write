fn main() {
    // App manifest (section 20): every app command must be listed here and is
    // denied unless a capability allows it explicitly. Keep this list in sync
    // with `commands::COMMAND_NAMES` (checked by tests/security_config.rs).
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "system_check",
        "library_status",
        "library_default_location",
        "library_choose_location",
        "library_create",
        "library_open",
        "library_reopen",
        "library_move",
        "library_cancel_move",
        "library_accept_location",
        "library_preferences",
        "library_update_preferences",
        "ui_state",
        "ui_update_settings",
        "ui_subscribe",
        "ui_menu_state",
        "ui_set_focus_mode",
        "ui_close_response",
        "documents_list",
        "document_create",
        "document_read",
        "document_save",
        "document_rename",
        "document_import",
        "document_copy",
        "document_set_favorite",
        "document_convert",
        "document_backlinks",
        "document_delete",
        "document_restore",
        "document_archive",
        "documents_trash",
        "document_purge",
        "trash_empty",
        "document_versions",
        "document_version_text",
        "document_version_save",
        "document_version_close",
        "document_version_restore",
        "recovery_write",
        "recovery_discard",
        "recovery_list",
        "recovery_restore",
        "recovery_drop",
        "session_save",
        "session_restore",
        "structure_state",
        "structure_document",
        "subnet_create",
        "subnet_update",
        "subnet_move",
        "subnet_delete",
        "document_assign",
        "document_unassign",
        "document_set_primary",
        "document_move",
        "tag_create",
        "tag_rename",
        "tag_merge",
        "tag_delete",
        "document_tag",
        "document_untag",
        "structure_undo",
        "structure_redo",
        "external_open",
        "external_read",
        "external_check",
        "external_keep_mine",
        "external_save",
        "external_save_as",
        "external_close",
        "external_import",
        "external_recent",
        "external_open_recent",
        "external_forget_recent",
        "open_requests_take",
        "theme_import",
        "theme_remove",
        "link_open",
    ]);

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
