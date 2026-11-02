//! Project Write – backend library.
//!
//! Layers (section 8): commands → services → database → SQLite. The frontend
//! reaches Rust only through the commands registered in [`commands::handler`].

#![deny(unsafe_code)]

pub mod commands;
pub mod database;
pub mod error;
pub mod services;
pub mod state;

use tauri::Manager;

// Test plugins exist only in debug builds (section 20).
#[cfg(all(feature = "e2e", not(debug_assertions)))]
compile_error!("the `e2e` feature embeds a WebDriver server and must never be part of a release build");

/// Starts the application.
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(|app| {
            let state = tauri::async_runtime::block_on(state::AppState::initialize())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running Project Write");
}
