//! Native folder dialog (section 8): "Für externe Dateien öffnet Rust den
//! Systemdialog und gibt eine Sitzungs-ID zurück. Das Frontend kann so keine
//! frei gewählten Pfade öffnen."
//!
//! The same rule applies to library folders: Rust shows the dialog, keeps
//! the chosen path and gives the frontend a token plus a display text.
//!
//! [`SystemPicker`] uses `rfd` (GTK on Linux, `IFileOpenDialog` on Windows,
//! `NSOpenPanel` on macOS) without any frontend permission.
//! [`ScriptedPicker`] answers from a list; Rust tests use it, and debug
//! builds with the `e2e` feature use it when `PW_E2E_PICK_FOLDERS` is set,
//! because WebDriver cannot operate native dialogs.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::oneshot;

/// What the dialog is for (decides the title).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickRequest {
    pub title: String,
    pub start_in: Option<PathBuf>,
}

/// Shows a folder dialog; resolves to `None` if the user cancels.
pub trait FolderPicker: Send + Sync {
    fn pick_folder(&self, request: PickRequest) -> oneshot::Receiver<Option<PathBuf>>;
}

/// The operating system's folder dialog, parented to the main window.
pub struct SystemPicker<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> SystemPicker<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> FolderPicker for SystemPicker<R> {
    fn pick_folder(&self, request: PickRequest) -> oneshot::Receiver<Option<PathBuf>> {
        let (sender, receiver) = oneshot::channel();
        let window = self.app.get_webview_window("main");
        // Dialogs must be created on the main (UI) thread; waiting for the
        // answer happens on a helper thread so the UI keeps running.
        let posted = self.app.run_on_main_thread(move || {
            let mut dialog = rfd::AsyncFileDialog::new().set_title(&request.title);
            if let Some(dir) = request.start_in.filter(|dir| dir.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(window) = &window {
                dialog = dialog.set_parent(window);
            }
            let answer = dialog.pick_folder();
            std::thread::spawn(move || {
                let picked = tauri::async_runtime::block_on(answer);
                let _ = sender.send(picked.map(|handle| handle.path().to_path_buf()));
            });
        });
        if let Err(error) = posted {
            log::error!("folder dialog could not be shown: {error}");
        }
        receiver
    }
}

/// Answers dialogs from a fixed list, then with "cancelled".
#[derive(Debug, Default)]
pub struct ScriptedPicker {
    answers: Mutex<VecDeque<Option<PathBuf>>>,
    requests: Mutex<Vec<PickRequest>>,
}

impl ScriptedPicker {
    pub fn new(answers: impl IntoIterator<Item = Option<PathBuf>>) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Adds an answer for the next dialog.
    pub fn push(&self, answer: Option<PathBuf>) {
        self.answers.lock().expect("picker lock").push_back(answer);
    }

    /// The dialogs that were requested so far.
    pub fn requests(&self) -> Vec<PickRequest> {
        self.requests.lock().expect("picker lock").clone()
    }
}

impl FolderPicker for ScriptedPicker {
    fn pick_folder(&self, request: PickRequest) -> oneshot::Receiver<Option<PathBuf>> {
        self.requests.lock().expect("picker lock").push(request);
        let answer = self
            .answers
            .lock()
            .expect("picker lock")
            .pop_front()
            .flatten();
        let (sender, receiver) = oneshot::channel();
        let _ = sender.send(answer);
        receiver
    }
}
