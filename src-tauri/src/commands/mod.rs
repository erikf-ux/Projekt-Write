//! Tauri commands: the only entry points from the frontend into Rust.
//!
//! Every command
//! * takes its payload as [`input::Input`] (validated, uniform errors),
//! * returns `Result<T, CommandError>` with `T` exported to TypeScript,
//! * is listed in [`COMMAND_NAMES`], in the app manifest in `build.rs` and in
//!   the window capability (checked by `tests/security_config.rs`).

pub mod input;
pub mod system;

use tauri::Runtime;
use tauri::ipc::Invoke;

/// Names of all registered app commands.
pub const COMMAND_NAMES: &[&str] = &["system_check"];

/// The invoke handler with every app command.
pub fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![system::system_check]
}
