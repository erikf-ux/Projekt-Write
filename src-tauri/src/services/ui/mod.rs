//! Interface settings and window state (section 11) on top of
//! `settings.json`, which the library session uses as well; the event
//! channel to the frontend (menu clicks, closing the window).

pub mod menu;
pub mod window;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use ts_rs::TS;

use menu::{MenuCommand, MenuItemState, MenuState};

use crate::error::{CommandError, ErrorCode};
use crate::services::locale::Locale;
use crate::services::session::lock;
use crate::services::settings::app::{AppSettingsError, AppSettingsStore};
use crate::services::settings::ui::{UiSettings, WindowState};

/// Operating system, for platform conventions in the interface (on macOS
/// `Cmd` replaces `Ctrl`, section 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Linux
        }
    }
}

/// What the frontend needs to set up the interface.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UiState {
    pub settings: UiSettings,
    /// Language of the interface: the setting, or the system's language.
    pub locale: Locale,
    pub platform: Platform,
    /// `settings.json` comes from a newer version; changes are refused.
    pub read_only: bool,
}

/// Messages from Rust to the frontend, sent through the channel the
/// frontend registers with `ui_subscribe` (no event permission needed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum UiEvent {
    /// A menu item was chosen.
    Command { command: MenuCommand },
    /// The user wants to close the window. The frontend saves what is
    /// pending and answers with `ui_close_response`.
    CloseRequested,
}

/// What happens with a close request of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// Let the window close.
    Close,
    /// Keep it open; the frontend was asked.
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseState {
    Idle,
    /// The frontend was asked and has not answered yet.
    Asked,
    /// The frontend agreed; the next close request closes.
    Confirmed,
}

pub struct UiService {
    settings: Arc<Mutex<AppSettingsStore>>,
    system_locale: Locale,
    events: Mutex<Option<Channel<UiEvent>>>,
    close: Mutex<CloseState>,
    menu: Mutex<MenuState>,
}

impl UiService {
    pub fn new(settings: Arc<Mutex<AppSettingsStore>>, system_locale: Locale) -> Self {
        Self {
            settings,
            system_locale,
            events: Mutex::new(None),
            close: Mutex::new(CloseState::Idle),
            menu: Mutex::new(MenuState::default()),
        }
    }

    /// Registers the frontend's event channel (again after a reload).
    pub fn subscribe(&self, channel: Channel<UiEvent>) {
        *lock(&self.events) = Some(channel);
        *lock(&self.close) = CloseState::Idle;
    }

    /// Sends `event`; `false` if no frontend listens.
    pub fn send(&self, event: UiEvent) -> bool {
        match lock(&self.events).as_ref() {
            Some(channel) => match channel.send(event) {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("event not delivered: {error}");
                    false
                }
            },
            None => false,
        }
    }

    /// A menu item was chosen; `false` if the frontend did not get it.
    pub fn menu_command(&self, id: &str) -> bool {
        match MenuCommand::from_id(id) {
            Some(command) => self.send(UiEvent::Command { command }),
            None => {
                log::warn!("unknown menu item {id}");
                false
            }
        }
    }

    /// Decides about a close request of the window. Without a listening
    /// frontend, and on a second request while the first is pending, the
    /// window closes: the user is never trapped.
    pub fn close_requested(&self) -> CloseDecision {
        let mut close = lock(&self.close);
        match *close {
            CloseState::Confirmed => CloseDecision::Close,
            CloseState::Asked => {
                log::info!("second close request; closing");
                CloseDecision::Close
            }
            CloseState::Idle => {
                if self.send(UiEvent::CloseRequested) {
                    *close = CloseState::Asked;
                    CloseDecision::Wait
                } else {
                    CloseDecision::Close
                }
            }
        }
    }

    /// The frontend's answer: close now, or stay open.
    pub fn close_response(&self, close: bool) {
        *lock(&self.close) = if close {
            CloseState::Confirmed
        } else {
            CloseState::Idle
        };
    }

    /// Stores the state of the menu items reported by the frontend.
    pub fn set_menu_state(&self, items: BTreeMap<MenuCommand, MenuItemState>) -> MenuState {
        let mut menu = lock(&self.menu);
        menu.set(items);
        menu.clone()
    }

    pub fn menu_state(&self) -> MenuState {
        lock(&self.menu).clone()
    }

    pub fn state(&self) -> UiState {
        let store = lock(&self.settings);
        let settings = store.get().ui.clone();
        UiState {
            locale: Locale::effective(settings.language, self.system_locale),
            settings,
            platform: Platform::current(),
            read_only: store.is_read_only(),
        }
    }

    pub fn locale(&self) -> Locale {
        self.state().locale
    }

    /// Replaces the interface settings (already validated) and writes them.
    /// Returns the new state and the previous settings.
    pub fn update(&self, next: UiSettings) -> Result<(UiState, UiSettings), CommandError> {
        let previous = {
            let mut store = lock(&self.settings);
            let previous = store.get().ui.clone();
            store
                .update(|settings| settings.ui = next)
                .map_err(settings_error)?;
            previous
        };
        Ok((self.state(), previous))
    }

    pub fn window_state(&self) -> Option<WindowState> {
        lock(&self.settings).get().window
    }

    /// Remembers the window state; a failure is only logged (the window
    /// opens at its default size next time).
    pub fn save_window_state(&self, state: WindowState) {
        let mut store = lock(&self.settings);
        if store.get().window == Some(state) || store.is_read_only() {
            return;
        }
        if let Err(error) = store.update(|settings| settings.window = Some(state)) {
            log::warn!("window state not saved: {error}");
        }
    }
}

fn settings_error(error: AppSettingsError) -> CommandError {
    match error {
        AppSettingsError::ReadOnly => CommandError::new(ErrorCode::SettingsReadOnly),
        AppSettingsError::Io(error) => {
            log::error!("settings not saved: {error}");
            CommandError::new(ErrorCode::SettingsNotSaved)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use tauri::ipc::{Channel, InvokeResponseBody};

    use super::*;
    use crate::clock::Timestamp;

    fn service() -> (UiService, tempfile::TempDir) {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        let (store, _) = AppSettingsStore::load(dir.path(), Timestamp::now());
        (
            UiService::new(Arc::new(Mutex::new(store)), Locale::En),
            dir,
        )
    }

    /// A channel that records what Rust sends.
    fn recorder() -> (Channel<UiEvent>, Arc<Mutex<Vec<serde_json::Value>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        });
        (channel, seen)
    }

    #[test]
    fn without_a_frontend_the_window_closes_at_once() {
        let (ui, _dir) = service();
        assert_eq!(ui.close_requested(), CloseDecision::Close);
        assert!(!ui.menu_command("palette.show"));
    }

    #[test]
    fn closing_waits_for_the_frontend_once() {
        let (ui, _dir) = service();
        let (channel, seen) = recorder();
        ui.subscribe(channel);

        assert_eq!(ui.close_requested(), CloseDecision::Wait);
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [serde_json::json!({ "kind": "closeRequested" })]
        );
        // A second click on the close button while the frontend saves.
        assert_eq!(ui.close_requested(), CloseDecision::Close);

        // The frontend stays open (e.g. the user cancelled): ask again later.
        ui.close_response(false);
        assert_eq!(ui.close_requested(), CloseDecision::Wait);
        ui.close_response(true);
        assert_eq!(ui.close_requested(), CloseDecision::Close);
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn menu_clicks_reach_the_frontend_as_commands() {
        let (ui, _dir) = service();
        let (channel, seen) = recorder();
        ui.subscribe(channel);
        assert!(ui.menu_command("palette.show"));
        assert!(!ui.menu_command("document.delete"));
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [serde_json::json!({ "kind": "command", "command": "palette.show" })]
        );
    }
}
