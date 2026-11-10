//! Native folder and file dialogs (section 8): "Für externe Dateien öffnet
//! Rust den Systemdialog und gibt eine Sitzungs-ID zurück. Das Frontend kann
//! so keine frei gewählten Pfade öffnen."
//!
//! The same rule applies to library folders: Rust shows the dialog, keeps
//! the chosen path and gives the frontend a token plus a display text.
//!
//! [`SystemPicker`] uses `rfd` (GTK on Linux, `IFileOpenDialog` and
//! `IFileSaveDialog` on Windows, `NSOpenPanel` on macOS) without any
//! frontend permission. [`ScriptedPicker`] answers from a list; Rust tests
//! use it, and debug builds with the `e2e` feature use it when
//! `PW_E2E_PICK_FOLDERS` or `PW_E2E_PICK_FILES` is set, because WebDriver
//! cannot operate native dialogs.

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

/// A file type offered by the file dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    /// Without dot, for example `md`.
    pub extensions: Vec<String>,
}

/// An open or save dialog for one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePickRequest {
    pub title: String,
    pub start_in: Option<PathBuf>,
    /// Suggested name (save dialog).
    pub file_name: Option<String>,
    pub filters: Vec<FileFilter>,
}

/// Which file dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDialog {
    Open,
    Save,
}

/// Shows a file dialog; resolves to `None` if the user cancels.
pub trait FilePicker: Send + Sync {
    fn pick_file(
        &self,
        dialog: FileDialog,
        request: FilePickRequest,
    ) -> oneshot::Receiver<Option<PathBuf>>;

    /// An open dialog for several files. By default one file of
    /// [`Self::pick_file`] (the scripted picker of the tests).
    fn pick_files(&self, request: FilePickRequest) -> oneshot::Receiver<Option<Vec<PathBuf>>> {
        let single = self.pick_file(FileDialog::Open, request);
        let (sender, receiver) = oneshot::channel();
        tauri::async_runtime::spawn(async move {
            let picked = single.await.ok().flatten().map(|path| vec![path]);
            let _ = sender.send(picked);
        });
        receiver
    }
}

/// The operating system's folder and file dialogs, parented to the main
/// window.
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

impl<R: Runtime> FilePicker for SystemPicker<R> {
    fn pick_files(&self, request: FilePickRequest) -> oneshot::Receiver<Option<Vec<PathBuf>>> {
        let (sender, receiver) = oneshot::channel();
        let window = self.app.get_webview_window("main");
        let posted = self.app.run_on_main_thread(move || {
            let mut dialog = rfd::AsyncFileDialog::new().set_title(&request.title);
            if let Some(dir) = request.start_in.filter(|dir| dir.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            for filter in &request.filters {
                dialog = dialog.add_filter(&filter.name, &filter.extensions);
            }
            if let Some(window) = &window {
                dialog = dialog.set_parent(window);
            }
            let answer = dialog.pick_files();
            std::thread::spawn(move || {
                let picked = tauri::async_runtime::block_on(answer);
                let _ = sender.send(picked.map(|handles| {
                    handles
                        .iter()
                        .map(|handle| handle.path().to_path_buf())
                        .collect()
                }));
            });
        });
        if let Err(error) = posted {
            log::error!("file dialog could not be shown: {error}");
        }
        receiver
    }

    fn pick_file(
        &self,
        kind: FileDialog,
        request: FilePickRequest,
    ) -> oneshot::Receiver<Option<PathBuf>> {
        let (sender, receiver) = oneshot::channel();
        let window = self.app.get_webview_window("main");
        let posted = self.app.run_on_main_thread(move || {
            let mut dialog = rfd::AsyncFileDialog::new().set_title(&request.title);
            if let Some(dir) = request.start_in.filter(|dir| dir.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = &request.file_name {
                dialog = dialog.set_file_name(name);
            }
            for filter in &request.filters {
                dialog = dialog.add_filter(&filter.name, &filter.extensions);
            }
            if let Some(window) = &window {
                dialog = dialog.set_parent(window);
            }
            let answer = match kind {
                FileDialog::Open => futures_box(dialog.pick_file()),
                FileDialog::Save => futures_box(dialog.save_file()),
            };
            std::thread::spawn(move || {
                let picked = tauri::async_runtime::block_on(answer);
                let _ = sender.send(picked.map(|handle| handle.path().to_path_buf()));
            });
        });
        if let Err(error) = posted {
            log::error!("file dialog could not be shown: {error}");
        }
        receiver
    }
}

type PickedFile = std::pin::Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>>;

fn futures_box(
    future: impl Future<Output = Option<rfd::FileHandle>> + Send + 'static,
) -> PickedFile {
    Box::pin(future)
}

/// Answers dialogs from a fixed list, then with "cancelled".
#[derive(Debug, Default)]
pub struct ScriptedPicker {
    answers: Mutex<VecDeque<Option<PathBuf>>>,
    requests: Mutex<Vec<PickRequest>>,
    file_requests: Mutex<Vec<(FileDialog, FilePickRequest)>>,
}

impl ScriptedPicker {
    pub fn new(answers: impl IntoIterator<Item = Option<PathBuf>>) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            file_requests: Mutex::new(Vec::new()),
        }
    }

    /// Adds an answer for the next dialog.
    pub fn push(&self, answer: Option<PathBuf>) {
        self.answers.lock().expect("picker lock").push_back(answer);
    }

    /// The folder dialogs that were requested so far.
    pub fn requests(&self) -> Vec<PickRequest> {
        self.requests.lock().expect("picker lock").clone()
    }

    /// The file dialogs that were requested so far.
    pub fn file_requests(&self) -> Vec<(FileDialog, FilePickRequest)> {
        self.file_requests.lock().expect("picker lock").clone()
    }

    fn next_answer(&self) -> oneshot::Receiver<Option<PathBuf>> {
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

impl FolderPicker for ScriptedPicker {
    fn pick_folder(&self, request: PickRequest) -> oneshot::Receiver<Option<PathBuf>> {
        self.requests.lock().expect("picker lock").push(request);
        self.next_answer()
    }
}

impl FilePicker for ScriptedPicker {
    fn pick_file(
        &self,
        dialog: FileDialog,
        request: FilePickRequest,
    ) -> oneshot::Receiver<Option<PathBuf>> {
        self.file_requests
            .lock()
            .expect("picker lock")
            .push((dialog, request));
        self.next_answer()
    }
}
