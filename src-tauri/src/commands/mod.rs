//! Tauri commands: the only entry points from the frontend into Rust.
//!
//! Every command
//! * takes its payload as [`input::Input`] (validated, uniform errors),
//! * returns `Result<T, CommandError>` with `T` exported to TypeScript,
//! * is listed in [`COMMAND_NAMES`], in the app manifest in `build.rs` and in
//!   the window capability (checked by `tests/security_config.rs`).

pub mod input;
pub mod library;
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
    ]
}
