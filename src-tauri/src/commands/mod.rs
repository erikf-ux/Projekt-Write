//! Tauri commands: the only entry points from the frontend into Rust.
//!
//! Every command
//! * takes its payload as [`input::Input`] (validated, uniform errors),
//! * returns `Result<T, CommandError>` with `T` exported to TypeScript,
//! * is listed in [`COMMAND_NAMES`], in the app manifest in `build.rs` and in
//!   the window capability (checked by `tests/security_config.rs`).

pub mod documents;
pub mod external;
pub mod input;
pub mod library;
pub mod links;
pub mod system;
pub mod ui;

use tauri::Runtime;
use tauri::ipc::Invoke;

/// Names of all registered app commands.
pub const COMMAND_NAMES: &[&str] = &[
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
];

/// The invoke handler with every app command.
pub fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        system::system_check,
        library::library_status,
        library::library_default_location,
        library::library_choose_location,
        library::library_create,
        library::library_open,
        library::library_reopen,
        library::library_move,
        library::library_cancel_move,
        library::library_accept_location,
        ui::ui_state,
        ui::ui_update_settings,
        ui::ui_subscribe,
        ui::ui_menu_state,
        ui::ui_set_focus_mode,
        ui::ui_close_response,
        documents::documents_list,
        documents::document_create,
        documents::document_read,
        documents::document_save,
        documents::document_rename,
        documents::document_import,
        documents::document_copy,
        documents::document_set_favorite,
        documents::document_convert,
        external::external_open,
        external::external_read,
        external::external_check,
        external::external_keep_mine,
        external::external_save,
        external::external_save_as,
        external::external_close,
        external::external_import,
        external::external_recent,
        external::external_open_recent,
        external::external_forget_recent,
        external::open_requests_take,
        ui::theme_import,
        ui::theme_remove,
        links::link_open,
    ]
}
