//! Interface settings in `settings.json` (section 9, "Einstellungen";
//! section 11): language, design, zoom, side bar, line width, shortcuts and
//! the window state.
//!
//! Every value is checked twice: when the frontend changes it
//! ([`UiSettings::validate`]) and when the file is loaded. An invalid value in
//! the file is replaced by its default and logged, so one bad entry never
//! makes the whole file unusable (the library path stays intact).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::error::CommandError;

pub const MIN_ZOOM: f64 = 0.5;
pub const MAX_ZOOM: f64 = 2.0;
pub const MIN_SIDEBAR_WIDTH: u32 = 160;
pub const MAX_SIDEBAR_WIDTH: u32 = 640;
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 260;
/// Reading speed for the reading time (section 12: "200 Wörter pro Minute
/// (einstellbar)").
pub const DEFAULT_WORDS_PER_MINUTE: u32 = 200;
pub const MIN_WORDS_PER_MINUTE: u32 = 50;
pub const MAX_WORDS_PER_MINUTE: u32 = 1000;
/// Commands with their own shortcuts at most.
const MAX_BOUND_COMMANDS: usize = 300;
/// Shortcuts per command at most.
const MAX_KEYS_PER_COMMAND: usize = 4;
const MAX_COMMAND_ID_LEN: usize = 64;
const MAX_CHORD_LEN: usize = 40;
/// `custom:` followed by the 32 hex digits of an imported theme.
const CUSTOM_THEME_PREFIX: &str = "custom:";

/// Modifiers in their canonical order. On macOS `Ctrl` stands for `Cmd`
/// (section 11).
const MODIFIERS: [&str; 3] = ["Ctrl", "Alt", "Shift"];
/// Keys with a name instead of a character (`Plus` because `+` separates the
/// parts of a chord).
const NAMED_KEYS: [&str; 16] = [
    "Plus",
    "Space",
    "Enter",
    "Escape",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
];

/// Language of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum LanguageSetting {
    /// Follows the system (section 11: "standardmäßig passend zum System").
    #[default]
    System,
    De,
    En,
}

/// The side bar (Explorer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SidebarSettings {
    pub visible: bool,
    /// Width in CSS pixels.
    pub width: u32,
}

impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: DEFAULT_SIDEBAR_WIDTH,
        }
    }
}

/// The reading time of the writing statistics (section 12: "Lesezeit als
/// optionale Anzeige").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ReadingTimeSettings {
    /// Shown in the status bar and the statistics.
    pub visible: bool,
    pub words_per_minute: u32,
}

impl Default for ReadingTimeSettings {
    fn default() -> Self {
        Self {
            visible: false,
            words_per_minute: DEFAULT_WORDS_PER_MINUTE,
        }
    }
}

/// The interface settings the frontend reads and changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UiSettings {
    pub language: LanguageSetting,
    /// `system`, `light`, `dark` or `custom:<id>` of an imported theme.
    pub theme: String,
    /// Zoom factor of the interface, 0.5 to 2.
    pub zoom: f64,
    pub sidebar: SidebarSettings,
    /// Lines of the writing surface at most 72 characters wide (section 11).
    pub limit_line_width: bool,
    pub reading_time: ReadingTimeSettings,
    /// Shortcuts that differ from the defaults: command id → chords such as
    /// `Ctrl+Shift+P`. An empty list removes every shortcut of the command.
    pub keybindings: BTreeMap<String, Vec<String>>,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            language: LanguageSetting::System,
            theme: "system".to_owned(),
            zoom: 1.0,
            sidebar: SidebarSettings::default(),
            limit_line_width: true,
            reading_time: ReadingTimeSettings::default(),
            keybindings: BTreeMap::new(),
        }
    }
}

/// Keys of [`UiSettings`] in `settings.json` (top level, next to
/// `libraryPath`).
const KEY_LANGUAGE: &str = "language";
const KEY_THEME: &str = "theme";
const KEY_ZOOM: &str = "zoom";
const KEY_SIDEBAR: &str = "sidebar";
const KEY_LIMIT_LINE_WIDTH: &str = "limitLineWidth";
const KEY_READING_TIME: &str = "readingTime";
const KEY_KEYBINDINGS: &str = "keybindings";
const KEY_WINDOW: &str = "window";

impl UiSettings {
    /// Checks every value (for changes coming from the frontend).
    pub fn validate(&self) -> Result<(), CommandError> {
        validate_theme(&self.theme)
            .map_err(|reason| CommandError::invalid_input("theme", reason))?;
        validate_zoom(self.zoom).map_err(|reason| CommandError::invalid_input("zoom", reason))?;
        validate_sidebar(&self.sidebar)
            .map_err(|reason| CommandError::invalid_input("sidebar", reason))?;
        validate_reading_time(&self.reading_time)
            .map_err(|reason| CommandError::invalid_input("readingTime", reason))?;
        validate_keybindings(&self.keybindings)
            .map_err(|reason| CommandError::invalid_input("keybindings", reason))?;
        Ok(())
    }

    /// Takes the interface settings out of the top-level object of
    /// `settings.json`. Missing or invalid values get their defaults.
    pub fn take_from(object: &mut Map<String, Value>) -> Self {
        let defaults = Self::default();
        Self {
            language: take(object, KEY_LANGUAGE, |_| Ok(())).unwrap_or(defaults.language),
            theme: take(object, KEY_THEME, |theme: &String| validate_theme(theme))
                .unwrap_or(defaults.theme),
            zoom: take(object, KEY_ZOOM, |zoom: &f64| validate_zoom(*zoom))
                .unwrap_or(defaults.zoom),
            sidebar: take(object, KEY_SIDEBAR, validate_sidebar).unwrap_or(defaults.sidebar),
            limit_line_width: take(object, KEY_LIMIT_LINE_WIDTH, |_| Ok(()))
                .unwrap_or(defaults.limit_line_width),
            reading_time: take(object, KEY_READING_TIME, validate_reading_time)
                .unwrap_or(defaults.reading_time),
            keybindings: take(object, KEY_KEYBINDINGS, validate_keybindings)
                .unwrap_or(defaults.keybindings),
        }
    }

    /// Writes the interface settings into the top-level object.
    pub fn put_into(&self, object: &mut Map<String, Value>) {
        let value = serde_json::to_value(self).expect("UiSettings serialize");
        if let Value::Object(fields) = value {
            object.extend(fields);
        }
    }
}

/// Size and position of the main window (physical pixels), restored at
/// start. Rust writes it; the frontend never sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

pub const MIN_WINDOW_WIDTH: u32 = 400;
pub const MIN_WINDOW_HEIGHT: u32 = 300;
const MAX_WINDOW_EDGE: u32 = 16_384;
const MAX_WINDOW_COORDINATE: i32 = 65_536;

impl WindowState {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(MIN_WINDOW_WIDTH..=MAX_WINDOW_EDGE).contains(&self.width)
            || !(MIN_WINDOW_HEIGHT..=MAX_WINDOW_EDGE).contains(&self.height)
        {
            return Err("size");
        }
        if self.x.abs() > MAX_WINDOW_COORDINATE || self.y.abs() > MAX_WINDOW_COORDINATE {
            return Err("position");
        }
        Ok(())
    }

    /// Takes the window state out of the top-level object of `settings.json`.
    pub fn take_from(object: &mut Map<String, Value>) -> Option<Self> {
        take(object, KEY_WINDOW, Self::validate)
    }

    pub fn put_into(state: Option<&Self>, object: &mut Map<String, Value>) {
        if let Some(state) = state {
            object.insert(
                KEY_WINDOW.to_owned(),
                serde_json::to_value(state).expect("WindowState serialize"),
            );
        }
    }
}

/// Removes `key` from `object` and returns its value if it has the right
/// type and passes `check`; otherwise logs and returns `None`.
fn take<T: serde::de::DeserializeOwned>(
    object: &mut Map<String, Value>,
    key: &str,
    check: impl FnOnce(&T) -> Result<(), &'static str>,
) -> Option<T> {
    let value = object.remove(key)?;
    match serde_json::from_value::<T>(value) {
        Ok(parsed) => match check(&parsed) {
            Ok(()) => Some(parsed),
            Err(reason) => {
                log::warn!("setting {key} invalid ({reason}); default used");
                None
            }
        },
        Err(error) => {
            log::warn!("setting {key} invalid ({error}); default used");
            None
        }
    }
}

fn validate_theme(theme: &str) -> Result<(), &'static str> {
    match theme {
        "system" | "light" | "dark" => Ok(()),
        _ => {
            let id = theme
                .strip_prefix(CUSTOM_THEME_PREFIX)
                .ok_or("unknown theme")?;
            if is_theme_id(id) {
                Ok(())
            } else {
                Err("theme id")
            }
        }
    }
}

/// Ids of imported themes: 32 lower-case hex digits.
pub fn is_theme_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_zoom(zoom: f64) -> Result<(), &'static str> {
    if zoom.is_finite() && (MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
        Ok(())
    } else {
        Err("zoom range")
    }
}

fn validate_sidebar(sidebar: &SidebarSettings) -> Result<(), &'static str> {
    if (MIN_SIDEBAR_WIDTH..=MAX_SIDEBAR_WIDTH).contains(&sidebar.width) {
        Ok(())
    } else {
        Err("width range")
    }
}

fn validate_reading_time(reading_time: &ReadingTimeSettings) -> Result<(), &'static str> {
    if (MIN_WORDS_PER_MINUTE..=MAX_WORDS_PER_MINUTE).contains(&reading_time.words_per_minute) {
        Ok(())
    } else {
        Err("words per minute range")
    }
}

fn validate_keybindings(bindings: &BTreeMap<String, Vec<String>>) -> Result<(), &'static str> {
    if bindings.len() > MAX_BOUND_COMMANDS {
        return Err("too many commands");
    }
    for (command, chords) in bindings {
        validate_command_id(command)?;
        if chords.len() > MAX_KEYS_PER_COMMAND {
            return Err("too many shortcuts");
        }
        for (index, chord) in chords.iter().enumerate() {
            validate_chord(chord)?;
            if chords[..index].contains(chord) {
                return Err("duplicate shortcut");
            }
        }
    }
    Ok(())
}

/// Command ids look like `palette.show` or `document.new`.
pub fn validate_command_id(id: &str) -> Result<(), &'static str> {
    let valid = !id.is_empty()
        && id.len() <= MAX_COMMAND_ID_LEN
        && id.split('.').count() >= 2
        && id.split('.').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_alphanumeric())
        });
    if valid { Ok(()) } else { Err("command id") }
}

/// Checks the canonical form of a shortcut, as the frontend writes it:
/// modifiers in the order `Ctrl`, `Alt`, `Shift`, then one key.
///
/// * A character key is upper case if it has case (`Ctrl+K`, not `Ctrl+k`).
/// * `Shift` combines only with letters and named keys; for other characters
///   the character itself already reflects `Shift` (`Ctrl+?`).
/// * Every key except `F1`–`F24` needs `Ctrl` or `Alt`, so a shortcut can
///   never swallow normal typing.
pub fn validate_chord(chord: &str) -> Result<(), &'static str> {
    if chord.is_empty() || chord.len() > MAX_CHORD_LEN {
        return Err("chord length");
    }
    let parts: Vec<&str> = chord.split('+').collect();
    let (key, modifiers) = parts.split_last().ok_or("chord empty")?;
    let mut previous: Option<usize> = None;
    for modifier in modifiers {
        let index = MODIFIERS
            .iter()
            .position(|known| known == modifier)
            .ok_or("unknown modifier")?;
        if previous.is_some_and(|before| index <= before) {
            return Err("modifier order");
        }
        previous = Some(index);
    }
    let has = |name: &str| modifiers.contains(&name);
    let command_modifier = has("Ctrl") || has("Alt");

    if is_function_key(key) {
        return Ok(());
    }
    if *key == "Plus" {
        // `+` is a character: Shift is part of typing it.
        return match (command_modifier, has("Shift")) {
            (true, false) => Ok(()),
            (_, true) => Err("Shift with a character"),
            (false, false) => Err("needs Ctrl or Alt"),
        };
    }
    if NAMED_KEYS.contains(key) {
        return if command_modifier {
            Ok(())
        } else {
            Err("needs Ctrl or Alt")
        };
    }
    let mut chars = key.chars();
    let (Some(character), None) = (chars.next(), chars.next()) else {
        return Err("unknown key");
    };
    if character.is_control() || character.is_whitespace() || character == '+' {
        return Err("unknown key");
    }
    let has_case = character.is_lowercase() || character.is_uppercase();
    // Canonical letters are upper case; `ß` and other letters without a
    // single upper-case form stay as they are.
    if character.is_lowercase() && character.to_uppercase().count() == 1 {
        return Err("key not upper case");
    }
    if has("Shift") && !has_case {
        return Err("Shift with a character");
    }
    if command_modifier {
        Ok(())
    } else {
        Err("needs Ctrl or Alt")
    }
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F')
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=24).contains(&number) && !key.starts_with("F0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_chords_are_accepted() {
        for chord in [
            "Ctrl+Shift+P",
            "Ctrl+N",
            "Ctrl+W",
            "F11",
            "Shift+F10",
            "Ctrl+Plus",
            "Ctrl+=",
            "Ctrl+-",
            "Ctrl+0",
            "Ctrl+,",
            "Ctrl+PageDown",
            "Ctrl+Shift+PageUp",
            "Alt+Ö",
            "Ctrl+ß",
            "Ctrl+Alt+Shift+K",
            "Ctrl+?",
        ] {
            assert_eq!(validate_chord(chord), Ok(()), "{chord}");
        }
    }

    #[test]
    fn invalid_chords_are_rejected() {
        for chord in [
            "",
            "P",
            "Shift+P",
            "Ctrl+p",
            "Shift+Ctrl+P",
            "Ctrl+Ctrl+P",
            "Meta+P",
            "Ctrl+",
            "Ctrl++",
            "Ctrl+Shift+=",
            "Ctrl+Shift+Plus",
            "Plus",
            "Ctrl+ ",
            "Ctrl+\u{7}",
            "Ctrl+AB",
            "Enter",
            "Escape",
            "Shift+Tab",
            "F0",
            "F25",
            "F01",
            "Ctrl+Space+K",
            &"Ctrl+".repeat(10),
        ] {
            assert!(validate_chord(chord).is_err(), "{chord:?} accepted");
        }
    }

    #[test]
    fn command_ids_need_namespace_and_plain_characters() {
        for id in [
            "palette.show",
            "document.new",
            "view.zoomIn",
            "tab.move.left",
        ] {
            assert_eq!(validate_command_id(id), Ok(()), "{id}");
        }
        for id in [
            "palette",
            "",
            "Palette.show",
            "palette.",
            ".show",
            "a.b-c",
            "a.ü",
        ] {
            assert!(validate_command_id(id).is_err(), "{id:?} accepted");
        }
    }

    #[test]
    fn validate_reports_the_field() {
        let mut settings = UiSettings {
            zoom: 3.0,
            ..UiSettings::default()
        };
        let error = settings.validate().unwrap_err();
        assert_eq!(error.details.unwrap()["field"], "zoom");

        settings.zoom = 1.0;
        settings.theme = "custom:XYZ".to_owned();
        assert_eq!(
            settings.validate().unwrap_err().details.unwrap()["field"],
            "theme"
        );

        settings.theme = format!("custom:{}", "a".repeat(32));
        settings.keybindings.insert(
            "palette.show".to_owned(),
            vec!["Ctrl+K".into(), "Ctrl+K".into()],
        );
        assert_eq!(
            settings.validate().unwrap_err().details.unwrap()["field"],
            "keybindings"
        );
        settings
            .keybindings
            .insert("palette.show".to_owned(), vec!["Ctrl+K".into()]);
        assert_eq!(settings.validate(), Ok(()));
    }

    #[test]
    fn loading_keeps_valid_values_and_replaces_invalid_ones() {
        let mut object = json!({
            "language": "de",
            "theme": "neon",
            "zoom": 1.25,
            "sidebar": { "visible": false, "width": 9000 },
            "limitLineWidth": "yes",
            "readingTime": { "visible": true, "wordsPerMinute": 20 },
            "keybindings": { "palette.show": ["Ctrl+K"] },
            "window": { "x": 10, "y": 20, "width": 900, "height": 700, "maximized": true },
            "future": 1
        })
        .as_object()
        .unwrap()
        .clone();
        let settings = UiSettings::take_from(&mut object);
        assert_eq!(settings.language, LanguageSetting::De);
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.zoom, 1.25);
        assert_eq!(settings.sidebar, SidebarSettings::default());
        assert!(settings.limit_line_width);
        // 20 words per minute is below the range: the default.
        assert_eq!(settings.reading_time, ReadingTimeSettings::default());
        assert_eq!(settings.keybindings["palette.show"], vec!["Ctrl+K"]);
        let window = WindowState::take_from(&mut object).unwrap();
        assert_eq!(window.width, 900);
        assert!(window.maximized);
        // Only unknown keys remain.
        assert_eq!(object.keys().collect::<Vec<_>>(), vec!["future"]);
    }

    #[test]
    fn invalid_window_states_are_dropped() {
        for window in [
            json!({ "x": 0, "y": 0, "width": 10, "height": 700, "maximized": false }),
            json!({ "x": 9_999_999, "y": 0, "width": 900, "height": 700, "maximized": false }),
            json!({ "x": 0, "y": 0, "width": 900, "height": 700 }),
            json!("big"),
        ] {
            let mut object = Map::new();
            object.insert("window".to_owned(), window.clone());
            assert_eq!(WindowState::take_from(&mut object), None, "{window}");
        }
    }

    #[test]
    fn settings_round_trip_through_the_object() {
        let mut settings = UiSettings {
            language: LanguageSetting::En,
            theme: "dark".to_owned(),
            zoom: 0.8,
            sidebar: SidebarSettings {
                visible: false,
                width: 300,
            },
            limit_line_width: false,
            reading_time: ReadingTimeSettings {
                visible: true,
                words_per_minute: 250,
            },
            keybindings: BTreeMap::new(),
        };
        settings
            .keybindings
            .insert("focus.toggle".to_owned(), vec![]);
        let mut object = Map::new();
        settings.put_into(&mut object);
        assert_eq!(object["limitLineWidth"], json!(false));
        assert_eq!(
            object["readingTime"],
            json!({ "visible": true, "wordsPerMinute": 250 })
        );
        assert_eq!(UiSettings::take_from(&mut object), settings);
        assert!(object.is_empty());
    }
}
