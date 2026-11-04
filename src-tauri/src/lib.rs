//! Project Write – backend library.
//!
//! Layers (section 8): commands → services → database / file system →
//! SQLite. The frontend reaches Rust only through the commands registered in
//! [`commands::handler`].

#![deny(unsafe_code)]

pub mod clock;
pub mod commands;
pub mod database;
pub mod error;
pub mod filesystem;
pub mod logging;
pub mod security;
pub mod services;
pub mod state;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, Runtime};

use clock::Generators;
use services::dialog::{FilePicker, FolderPicker, SystemPicker};
use services::session::AppDirs;
use state::AppState;

// Test plugins exist only in debug builds (section 20).
#[cfg(all(feature = "e2e", not(debug_assertions)))]
compile_error!(
    "the `e2e` feature embeds a WebDriver server and must never be part of a release build"
);

/// Starts the application.
pub fn run() {
    // Registered first: a second start hands over to the running instance and
    // ends before anything else happens (section 9).
    let builder = tauri::Builder::default().plugin(tauri_plugin_single_instance::init(
        |app, _arguments, _cwd| focus_main_window(app),
    ));

    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    let app = builder
        .setup(|app| {
            let dirs = app_dirs(app.handle())?;
            if let Err(error) = logging::init(&dirs.logs) {
                eprintln!("log file not available: {error}");
            }
            log::info!("Project Write {} starting", env!("CARGO_PKG_VERSION"));
            let picker = folder_picker(app.handle());
            let state = tauri::async_runtime::block_on(AppState::initialize(
                dirs,
                picker,
                file_picker(app.handle()),
                link_opener(),
                Generators::system(),
            ))?;
            let menu =
                services::ui::menu::build(app.handle(), state.ui.locale(), &state.ui.menu_state())?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_menu(menu.menu.clone())?;
                services::ui::window::apply_zoom(&window, state.ui.state().settings.zoom);
                services::ui::window::restore(&window, state.ui.window_state());
            }
            app.manage(services::ui::menu::MenuSlot(Mutex::new(Some(menu))));
            app.manage(state);
            Ok(())
        })
        .on_menu_event(|app, event| {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let id = event.id().as_ref();
            // Without a listening frontend, "Quit" still closes the window.
            if !state.ui.menu_command(id)
                && id == services::ui::menu::MenuCommand::AppQuit.id()
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.close();
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let Some(state) = window.app_handle().try_state::<AppState>() else {
                    return;
                };
                // The frontend saves what is pending first (section 9:
                // autosave "beim Schließen") and then confirms.
                match state.ui.close_requested() {
                    services::ui::CloseDecision::Wait => api.prevent_close(),
                    services::ui::CloseDecision::Close => {
                        remember_window_state(window.app_handle())
                    }
                }
            }
        })
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())
        .expect("error while building Project Write");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Close the database cleanly and release library.lock.
            if let Some(state) = handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(state.session.close());
            }
            log::info!("Project Write stopped");
        }
    });
}

/// Saves size, position and maximized state of the main window.
fn remember_window_state<R: Runtime>(app: &AppHandle<R>) {
    let (Some(window), Some(state)) = (app.get_webview_window("main"), app.try_state::<AppState>())
    else {
        return;
    };
    if let Some(current) = services::ui::window::capture(&window, state.ui.window_state()) {
        state.ui.save_window_state(current);
    }
}

/// Brings the window to the front (second start of the app).
fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Configuration, log and documents folders of the system.
///
/// Debug builds with the `e2e` feature use `PW_E2E_APP_DIRS` instead, so
/// end-to-end tests never touch the folders of the machine they run on.
fn app_dirs<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<AppDirs> {
    #[cfg(feature = "e2e")]
    if let Some(base) = std::env::var_os("PW_E2E_APP_DIRS") {
        let base = PathBuf::from(base);
        return Ok(AppDirs {
            config: base.join("config"),
            logs: base.join("logs"),
            documents: Some(base.join("documents")),
        });
    }
    let path = app.path();
    Ok(AppDirs {
        config: path.app_config_dir()?,
        logs: path.app_log_dir()?,
        documents: path.document_dir().ok().or_else(|| {
            path.home_dir()
                .ok()
                .map(|home| home.join(documents_folder_name()))
        }),
    })
}

fn documents_folder_name() -> PathBuf {
    PathBuf::from("Documents")
}

/// The system browser; in `e2e` builds with `PW_E2E_OPENED_LINKS` a recorder
/// that appends each address to that file.
fn link_opener() -> Arc<dyn services::links::LinkOpener> {
    #[cfg(feature = "e2e")]
    if let Some(file) = std::env::var_os("PW_E2E_OPENED_LINKS") {
        return Arc::new(services::links::RecordingOpener::with_file(PathBuf::from(
            file,
        )));
    }
    Arc::new(services::links::SystemOpener)
}

/// The native folder dialog; in `e2e` builds the answers listed in
/// `PW_E2E_PICK_FOLDERS` (an empty entry means "cancelled").
fn folder_picker<R: Runtime>(app: &AppHandle<R>) -> Arc<dyn FolderPicker> {
    #[cfg(feature = "e2e")]
    if let Some(list) = std::env::var_os("PW_E2E_PICK_FOLDERS") {
        let answers: Vec<Option<PathBuf>> = std::env::split_paths(&list)
            .map(|path| (!path.as_os_str().is_empty()).then_some(path))
            .collect();
        return Arc::new(services::dialog::ScriptedPicker::new(answers));
    }
    Arc::new(SystemPicker::new(app.clone()))
}

/// The native open and save dialogs; in `e2e` builds the answers listed in
/// `PW_E2E_PICK_FILES` (an empty entry means "cancelled").
fn file_picker<R: Runtime>(app: &AppHandle<R>) -> Arc<dyn FilePicker> {
    #[cfg(feature = "e2e")]
    if let Some(list) = std::env::var_os("PW_E2E_PICK_FILES") {
        let answers: Vec<Option<PathBuf>> = std::env::split_paths(&list)
            .map(|path| (!path.as_os_str().is_empty()).then_some(path))
            .collect();
        return Arc::new(services::dialog::ScriptedPicker::new(answers));
    }
    Arc::new(SystemPicker::new(app.clone()))
}
