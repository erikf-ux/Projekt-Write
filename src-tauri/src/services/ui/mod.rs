//! Interface settings and window state (section 11) on top of
//! `settings.json`, which the library session uses as well.

pub mod window;

use std::sync::{Arc, Mutex};

use serde::Serialize;
use ts_rs::TS;

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

pub struct UiService {
    settings: Arc<Mutex<AppSettingsStore>>,
    system_locale: Locale,
}

impl UiService {
    pub fn new(settings: Arc<Mutex<AppSettingsStore>>, system_locale: Locale) -> Self {
        Self {
            settings,
            system_locale,
        }
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
