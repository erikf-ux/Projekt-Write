//! The native menu bar (section 11: "Sie ist das native Tauri-Menü").
//!
//! Every item stands for a command of the frontend's command registry; a
//! click sends the command id to the frontend ([`super::UiEvent::Command`]),
//! which runs it exactly like the command palette or a shortcut would.
//!
//! * Labels come from the frontend's translation files (`src/i18n/*.json`),
//!   compiled in, so there is one set of texts.
//! * Items carry no native accelerators: shortcuts are handled once, in the
//!   webview, where they are configurable and testable (ADR 0012). On
//!   Windows the shortcut is shown after a tab, as Win32 menus do.
//! * The frontend reports which commands are available; the others are
//!   disabled.
//! * There is no Edit menu: the predefined edit items do nothing on Linux
//!   without libxdo (and never on Wayland). Cut, copy, paste and undo work
//!   with the keyboard in the writing surface.

use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};
use ts_rs::TS;

use crate::services::locale::Locale;

/// The commands the menu offers (ids of the frontend command registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum MenuCommand {
    #[serde(rename = "document.new")]
    DocumentNew,
    #[serde(rename = "document.save")]
    DocumentSave,
    #[serde(rename = "document.openFile")]
    DocumentOpenFile,
    #[serde(rename = "document.saveAs")]
    DocumentSaveAs,
    #[serde(rename = "tab.close")]
    TabClose,
    #[serde(rename = "settings.open")]
    SettingsOpen,
    #[serde(rename = "app.quit")]
    AppQuit,
    #[serde(rename = "palette.show")]
    PaletteShow,
    #[serde(rename = "sidebar.toggle")]
    SidebarToggle,
    #[serde(rename = "focus.toggle")]
    FocusToggle,
    #[serde(rename = "zoom.in")]
    ZoomIn,
    #[serde(rename = "zoom.out")]
    ZoomOut,
    #[serde(rename = "zoom.reset")]
    ZoomReset,
    #[serde(rename = "edit.undo")]
    EditUndo,
    #[serde(rename = "edit.redo")]
    EditRedo,
    #[serde(rename = "find.show")]
    FindShow,
    #[serde(rename = "replace.show")]
    ReplaceShow,
    #[serde(rename = "editor.toggleSource")]
    EditorToggleSource,
    #[serde(rename = "statistics.show")]
    StatisticsShow,
    #[serde(rename = "format.bold")]
    FormatBold,
    #[serde(rename = "format.italic")]
    FormatItalic,
    #[serde(rename = "format.underline")]
    FormatUnderline,
    #[serde(rename = "format.strike")]
    FormatStrike,
    #[serde(rename = "format.code")]
    FormatCode,
    #[serde(rename = "format.paragraph")]
    FormatParagraph,
    #[serde(rename = "format.heading1")]
    FormatHeading1,
    #[serde(rename = "format.heading2")]
    FormatHeading2,
    #[serde(rename = "format.heading3")]
    FormatHeading3,
    #[serde(rename = "format.bulletList")]
    FormatBulletList,
    #[serde(rename = "format.orderedList")]
    FormatOrderedList,
    #[serde(rename = "format.taskList")]
    FormatTaskList,
    #[serde(rename = "format.blockquote")]
    FormatBlockquote,
    #[serde(rename = "format.codeBlock")]
    FormatCodeBlock,
    #[serde(rename = "format.link")]
    FormatLink,
    #[serde(rename = "format.horizontalRule")]
    FormatHorizontalRule,
    #[serde(rename = "format.table")]
    FormatTable,
}

impl MenuCommand {
    pub const ALL: [MenuCommand; 36] = [
        MenuCommand::DocumentNew,
        MenuCommand::DocumentSave,
        MenuCommand::DocumentOpenFile,
        MenuCommand::DocumentSaveAs,
        MenuCommand::TabClose,
        MenuCommand::SettingsOpen,
        MenuCommand::AppQuit,
        MenuCommand::PaletteShow,
        MenuCommand::SidebarToggle,
        MenuCommand::FocusToggle,
        MenuCommand::ZoomIn,
        MenuCommand::ZoomOut,
        MenuCommand::ZoomReset,
        MenuCommand::EditUndo,
        MenuCommand::EditRedo,
        MenuCommand::FindShow,
        MenuCommand::ReplaceShow,
        MenuCommand::EditorToggleSource,
        MenuCommand::StatisticsShow,
        MenuCommand::FormatBold,
        MenuCommand::FormatItalic,
        MenuCommand::FormatUnderline,
        MenuCommand::FormatStrike,
        MenuCommand::FormatCode,
        MenuCommand::FormatParagraph,
        MenuCommand::FormatHeading1,
        MenuCommand::FormatHeading2,
        MenuCommand::FormatHeading3,
        MenuCommand::FormatBulletList,
        MenuCommand::FormatOrderedList,
        MenuCommand::FormatTaskList,
        MenuCommand::FormatBlockquote,
        MenuCommand::FormatCodeBlock,
        MenuCommand::FormatLink,
        MenuCommand::FormatHorizontalRule,
        MenuCommand::FormatTable,
    ];

    /// The command id, also the id of the menu item.
    pub fn id(self) -> &'static str {
        match self {
            MenuCommand::DocumentNew => "document.new",
            MenuCommand::DocumentSave => "document.save",
            MenuCommand::DocumentOpenFile => "document.openFile",
            MenuCommand::DocumentSaveAs => "document.saveAs",
            MenuCommand::TabClose => "tab.close",
            MenuCommand::SettingsOpen => "settings.open",
            MenuCommand::AppQuit => "app.quit",
            MenuCommand::PaletteShow => "palette.show",
            MenuCommand::SidebarToggle => "sidebar.toggle",
            MenuCommand::FocusToggle => "focus.toggle",
            MenuCommand::ZoomIn => "zoom.in",
            MenuCommand::ZoomOut => "zoom.out",
            MenuCommand::ZoomReset => "zoom.reset",
            MenuCommand::EditUndo => "edit.undo",
            MenuCommand::EditRedo => "edit.redo",
            MenuCommand::FindShow => "find.show",
            MenuCommand::ReplaceShow => "replace.show",
            MenuCommand::EditorToggleSource => "editor.toggleSource",
            MenuCommand::StatisticsShow => "statistics.show",
            MenuCommand::FormatBold => "format.bold",
            MenuCommand::FormatItalic => "format.italic",
            MenuCommand::FormatUnderline => "format.underline",
            MenuCommand::FormatStrike => "format.strike",
            MenuCommand::FormatCode => "format.code",
            MenuCommand::FormatParagraph => "format.paragraph",
            MenuCommand::FormatHeading1 => "format.heading1",
            MenuCommand::FormatHeading2 => "format.heading2",
            MenuCommand::FormatHeading3 => "format.heading3",
            MenuCommand::FormatBulletList => "format.bulletList",
            MenuCommand::FormatOrderedList => "format.orderedList",
            MenuCommand::FormatTaskList => "format.taskList",
            MenuCommand::FormatBlockquote => "format.blockquote",
            MenuCommand::FormatCodeBlock => "format.codeBlock",
            MenuCommand::FormatLink => "format.link",
            MenuCommand::FormatHorizontalRule => "format.horizontalRule",
            MenuCommand::FormatTable => "format.table",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    /// Translation key of the label (shared with the command palette).
    fn label_key(self) -> String {
        format!("command.{}", self.id())
    }

    /// Commands that work before the frontend has reported anything.
    fn initially_enabled(self) -> bool {
        matches!(self, MenuCommand::AppQuit)
    }
}

/// Longest shortcut text the frontend may set.
pub const MAX_SHORTCUT_LABEL_CHARS: usize = 32;

/// State of one item as the frontend reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MenuItemState {
    pub enabled: bool,
    /// The shortcut as shown to the user (`Strg+N`), if any.
    pub shortcut: Option<String>,
}

/// The state of all items.
#[derive(Debug, Clone, Default)]
pub struct MenuState {
    items: BTreeMap<MenuCommand, MenuItemState>,
}

impl MenuState {
    pub fn set(&mut self, items: BTreeMap<MenuCommand, MenuItemState>) {
        self.items = items;
    }

    fn get(&self, command: MenuCommand) -> MenuItemState {
        self.items.get(&command).cloned().unwrap_or(MenuItemState {
            enabled: command.initially_enabled(),
            shortcut: None,
        })
    }
}

type Translations = HashMap<String, String>;

static GERMAN: LazyLock<Translations> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../src/i18n/de.json")).expect("de.json")
});
static ENGLISH: LazyLock<Translations> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../src/i18n/en.json")).expect("en.json")
});

fn text(locale: Locale, key: &str) -> String {
    let translations = match locale {
        Locale::De => &*GERMAN,
        Locale::En => &*ENGLISH,
    };
    translations
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// Label of an item: the translated title and, on Windows, the shortcut
/// after a tab. GTK has no column for it, so Linux shows the title only.
fn item_label(locale: Locale, command: MenuCommand, state: &MenuItemState) -> String {
    let title = text(locale, &command.label_key());
    match &state.shortcut {
        Some(shortcut) if cfg!(windows) => format!("{title}\t{shortcut}"),
        _ => title,
    }
}

/// Menu structure: (submenu label key, items; `None` is a separator).
const LAYOUT: [(&str, &[Option<MenuCommand>]); 4] = [
    (
        "menu.file",
        &[
            Some(MenuCommand::DocumentNew),
            Some(MenuCommand::DocumentOpenFile),
            None,
            Some(MenuCommand::DocumentSave),
            Some(MenuCommand::DocumentSaveAs),
            Some(MenuCommand::TabClose),
            None,
            Some(MenuCommand::SettingsOpen),
            None,
            Some(MenuCommand::AppQuit),
        ],
    ),
    (
        "menu.edit",
        &[
            Some(MenuCommand::EditUndo),
            Some(MenuCommand::EditRedo),
            None,
            Some(MenuCommand::FindShow),
            Some(MenuCommand::ReplaceShow),
            None,
            Some(MenuCommand::EditorToggleSource),
            Some(MenuCommand::StatisticsShow),
        ],
    ),
    (
        "menu.format",
        &[
            Some(MenuCommand::FormatBold),
            Some(MenuCommand::FormatItalic),
            Some(MenuCommand::FormatUnderline),
            Some(MenuCommand::FormatStrike),
            Some(MenuCommand::FormatCode),
            None,
            Some(MenuCommand::FormatParagraph),
            Some(MenuCommand::FormatHeading1),
            Some(MenuCommand::FormatHeading2),
            Some(MenuCommand::FormatHeading3),
            None,
            Some(MenuCommand::FormatBulletList),
            Some(MenuCommand::FormatOrderedList),
            Some(MenuCommand::FormatTaskList),
            Some(MenuCommand::FormatBlockquote),
            Some(MenuCommand::FormatCodeBlock),
            None,
            Some(MenuCommand::FormatLink),
            Some(MenuCommand::FormatHorizontalRule),
            Some(MenuCommand::FormatTable),
        ],
    ),
    (
        "menu.view",
        &[
            Some(MenuCommand::PaletteShow),
            Some(MenuCommand::SidebarToggle),
            Some(MenuCommand::FocusToggle),
            None,
            Some(MenuCommand::ZoomIn),
            Some(MenuCommand::ZoomOut),
            Some(MenuCommand::ZoomReset),
        ],
    ),
];

/// The built menu and its items, to update them in place.
pub struct MenuHandles<R: Runtime> {
    pub menu: Menu<R>,
    submenus: Vec<(&'static str, Submenu<R>)>,
    items: Vec<(MenuCommand, MenuItem<R>)>,
}

/// Managed state: the menu of the main window (absent in tests without a
/// window menu).
pub struct MenuSlot<R: Runtime>(pub Mutex<Option<MenuHandles<R>>>);

impl<R: Runtime> MenuHandles<R> {
    /// Applies texts (after a language change) and enabled states.
    pub fn apply(&self, locale: Locale, state: &MenuState) {
        for (label_key, submenu) in &self.submenus {
            if let Err(error) = submenu.set_text(text(locale, label_key)) {
                log::warn!("menu text not set: {error}");
            }
        }
        for (command, item) in &self.items {
            let item_state = state.get(*command);
            if let Err(error) = item.set_text(item_label(locale, *command, &item_state)) {
                log::warn!("menu text not set: {error}");
            }
            if let Err(error) = item.set_enabled(item_state.enabled) {
                log::warn!("menu state not set: {error}");
            }
        }
    }
}

/// Builds the menu bar in `locale`.
pub fn build<R: Runtime>(
    app: &AppHandle<R>,
    locale: Locale,
    state: &MenuState,
) -> tauri::Result<MenuHandles<R>> {
    let mut items = Vec::new();
    let mut submenus = Vec::new();
    let menu = Menu::new(app)?;
    for (label_key, entries) in LAYOUT {
        let submenu = Submenu::new(app, text(locale, label_key), true)?;
        for entry in entries {
            match entry {
                Some(command) => {
                    let item_state = state.get(*command);
                    let item = MenuItem::with_id(
                        app,
                        command.id(),
                        item_label(locale, *command, &item_state),
                        item_state.enabled,
                        None::<&str>,
                    )?;
                    submenu.append(&item)?;
                    items.push((*command, item));
                }
                None => submenu.append(&PredefinedMenuItem::separator(app)?)?,
            }
        }
        menu.append(&submenu)?;
        submenus.push((label_key, submenu));
    }
    Ok(MenuHandles {
        menu,
        submenus,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_match_serde() {
        for command in MenuCommand::ALL {
            assert_eq!(MenuCommand::from_id(command.id()), Some(command));
            assert_eq!(
                serde_json::to_value(command).unwrap(),
                serde_json::Value::from(command.id())
            );
        }
        assert_eq!(MenuCommand::from_id("document.delete"), None);
    }

    #[test]
    fn every_label_is_translated_in_both_languages() {
        let mut keys: Vec<String> = LAYOUT.iter().map(|(key, _)| (*key).to_owned()).collect();
        keys.extend(MenuCommand::ALL.iter().map(|c| c.label_key()));
        for key in keys {
            for translations in [&*GERMAN, &*ENGLISH] {
                let label = translations
                    .get(&key)
                    .unwrap_or_else(|| panic!("{key} missing"));
                assert!(!label.trim().is_empty(), "{key} empty");
                assert!(!label.contains('\t'), "{key} contains a tab");
            }
        }
    }

    #[test]
    fn every_command_appears_exactly_once_in_the_layout() {
        let mut seen: Vec<MenuCommand> = LAYOUT
            .iter()
            .flat_map(|(_, entries)| entries.iter().flatten().copied())
            .collect();
        seen.sort();
        let mut all = MenuCommand::ALL.to_vec();
        all.sort();
        assert_eq!(seen, all);
    }

    #[test]
    fn shortcuts_are_shown_after_a_tab_on_windows_only() {
        let state = MenuItemState {
            enabled: true,
            shortcut: Some("Strg+N".to_owned()),
        };
        let label = item_label(Locale::De, MenuCommand::DocumentNew, &state);
        if cfg!(windows) {
            assert_eq!(label, "Neues Dokument\tStrg+N");
        } else {
            assert_eq!(label, "Neues Dokument");
        }
        assert_eq!(
            item_label(
                Locale::En,
                MenuCommand::DocumentNew,
                &MenuItemState::default()
            ),
            "New document"
        );
    }

    #[test]
    fn only_quit_works_before_the_frontend_reports() {
        let state = MenuState::default();
        for command in MenuCommand::ALL {
            assert_eq!(
                state.get(command).enabled,
                command == MenuCommand::AppQuit,
                "{command:?}"
            );
        }
    }
}
