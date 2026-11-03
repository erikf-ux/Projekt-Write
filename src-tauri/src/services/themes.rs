//! Importable themes (section 11: "Anpassbare Themes über Design-Tokens und
//! importierbare Theme-Dateien").
//!
//! A theme file is JSON with every design token as `#RRGGBB`:
//!
//! ```json
//! { "formatVersion": 1, "name": "Nacht", "base": "dark",
//!   "colors": { "surface": "#1E1E1E", "sidebar": "#252526", … } }
//! ```
//!
//! Rust checks the file completely, including the contrast rules of
//! sections 6 and 11, and keeps a normalised copy in `themes/` of the
//! configuration folder. The original file is only read (by the webview,
//! from a file the user picked).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::filesystem::atomic;
use crate::services::settings::ui::is_theme_id;

pub const THEME_FORMAT_VERSION: u32 = 1;
/// Largest theme file accepted.
pub const MAX_THEME_BYTES: usize = 64 * 1024;
pub const MAX_THEME_NAME_CHARS: usize = 60;
/// Imported themes at most.
pub const MAX_THEMES: usize = 50;
/// Section 6: text needs 4.5:1.
const MIN_TEXT_CONTRAST: f64 = 4.5;

/// Light or dark: decides the colour scheme of native controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ThemeBase {
    Dark,
    Light,
}

/// The design tokens of section 11 (as in `src/modules/themes/tokens.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ThemeColors {
    pub surface: String,
    pub sidebar: String,
    pub text: String,
    pub text_muted: String,
    pub accent: String,
    pub error: String,
    pub success: String,
    pub cluster_blue: String,
    pub cluster_green: String,
    pub cluster_violet: String,
    pub cluster_orange: String,
    pub cluster_grey: String,
}

impl ThemeColors {
    fn all(&self) -> [(&'static str, &String); 12] {
        [
            ("surface", &self.surface),
            ("sidebar", &self.sidebar),
            ("text", &self.text),
            ("textMuted", &self.text_muted),
            ("accent", &self.accent),
            ("error", &self.error),
            ("success", &self.success),
            ("clusterBlue", &self.cluster_blue),
            ("clusterGreen", &self.cluster_green),
            ("clusterViolet", &self.cluster_violet),
            ("clusterOrange", &self.cluster_orange),
            ("clusterGrey", &self.cluster_grey),
        ]
    }

    fn get(&self, token: &str) -> &str {
        self.all()
            .into_iter()
            .find(|(name, _)| *name == token)
            .map(|(_, value)| value.as_str())
            .expect("known token")
    }

    fn normalise(&mut self) {
        for value in [
            &mut self.surface,
            &mut self.sidebar,
            &mut self.text,
            &mut self.text_muted,
            &mut self.accent,
            &mut self.error,
            &mut self.success,
            &mut self.cluster_blue,
            &mut self.cluster_green,
            &mut self.cluster_violet,
            &mut self.cluster_orange,
            &mut self.cluster_grey,
        ] {
            *value = value.to_ascii_uppercase();
        }
    }
}

/// Text colours on their backgrounds (as `tokens.test.ts`), all 4.5:1.
const TEXT_PAIRS: [(&str, &str); 16] = [
    ("text", "surface"),
    ("textMuted", "surface"),
    ("accent", "surface"),
    ("error", "surface"),
    ("success", "surface"),
    ("text", "sidebar"),
    ("textMuted", "sidebar"),
    ("accent", "sidebar"),
    ("error", "sidebar"),
    ("success", "sidebar"),
    // Primary button: surface-coloured label on the accent colour.
    ("surface", "accent"),
    // Section 11: cluster colours on the writing surface.
    ("clusterBlue", "surface"),
    ("clusterGreen", "surface"),
    ("clusterViolet", "surface"),
    ("clusterOrange", "surface"),
    ("clusterGrey", "surface"),
];

/// An imported theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CustomTheme {
    /// 32 hex digits; the setting is `custom:<id>`.
    pub id: String,
    pub name: String,
    pub base: ThemeBase,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ThemeFile {
    format_version: u32,
    name: String,
    base: ThemeBase,
    colors: ThemeColors,
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    /// Not a theme file of this version; `field` names the problem.
    #[error("invalid theme file: {field}")]
    Invalid { field: String },
    /// A text colour does not reach 4.5:1 on its background.
    #[error("contrast of {pair} is {ratio:.2}:1")]
    Contrast { pair: String, ratio: f64 },
    #[error("too many themes")]
    TooMany,
    #[error("theme not found")]
    NotFound,
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn invalid(field: impl Into<String>) -> ThemeError {
    ThemeError::Invalid {
        field: field.into(),
    }
}

/// Parses and checks the content of a theme file.
pub fn parse(content: &str) -> Result<(String, ThemeBase, ThemeColors), ThemeError> {
    if content.len() > MAX_THEME_BYTES {
        return Err(invalid("size"));
    }
    let file: ThemeFile = serde_json::from_str(content).map_err(|error| {
        // serde names the offending field in its message; keep it short.
        let text = error.to_string();
        invalid(text.split(" at line").next().unwrap_or("format").to_owned())
    })?;
    if file.format_version != THEME_FORMAT_VERSION {
        return Err(invalid("formatVersion"));
    }
    let name = file.name.trim().to_owned();
    let length = name.chars().count();
    if length == 0 || length > MAX_THEME_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(invalid("name"));
    }
    let mut colors = file.colors;
    for (token, value) in colors.all() {
        if !is_hex_colour(value) {
            return Err(invalid(format!("colors.{token}")));
        }
    }
    colors.normalise();
    for (foreground, background) in TEXT_PAIRS {
        let ratio = contrast_ratio(colors.get(foreground), colors.get(background));
        if ratio < MIN_TEXT_CONTRAST {
            return Err(ThemeError::Contrast {
                pair: format!("{foreground}/{background}"),
                ratio,
            });
        }
    }
    Ok((name, file.base, colors))
}

fn is_hex_colour(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn channel(value: u8) -> f64 {
    let c = f64::from(value) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance of `#RRGGBB`.
fn luminance(hex: &str) -> f64 {
    let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).unwrap_or(0);
    0.2126 * channel(byte(1)) + 0.7152 * channel(byte(3)) + 0.0722 * channel(byte(5))
}

/// WCAG 2.x contrast ratio.
pub fn contrast_ratio(foreground: &str, background: &str) -> f64 {
    let a = luminance(foreground);
    let b = luminance(background);
    let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

/// The imported themes in `themes/` of the configuration folder.
#[derive(Debug, Clone)]
pub struct ThemeStore {
    dir: PathBuf,
}

impl ThemeStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            dir: config_dir.join("themes"),
        }
    }

    /// All valid themes, by name. Files that do not pass the checks (for
    /// example edited by hand) are skipped and logged, never deleted.
    pub fn list(&self) -> Vec<CustomTheme> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut themes: Vec<CustomTheme> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json")?;
                if !is_theme_id(id) || !entry.file_type().ok()?.is_file() {
                    return None;
                }
                self.load(id)
                    .map_err(|error| log::warn!("theme {id} skipped: {error}"))
                    .ok()
            })
            .collect();
        themes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        themes
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn load(&self, id: &str) -> Result<CustomTheme, ThemeError> {
        let data = atomic::read_limited(&self.path(id), MAX_THEME_BYTES as u64)?;
        let content = String::from_utf8(data).map_err(|_| invalid("encoding"))?;
        let (name, base, colors) = parse(&content)?;
        Ok(CustomTheme {
            id: id.to_owned(),
            name,
            base,
            colors,
        })
    }

    /// Checks `content` and stores a normalised copy under a new id.
    pub fn import(&self, content: &str, id: String) -> Result<CustomTheme, ThemeError> {
        let (name, base, colors) = parse(content)?;
        if self.list().len() >= MAX_THEMES {
            return Err(ThemeError::TooMany);
        }
        let theme = CustomTheme {
            id,
            name,
            base,
            colors,
        };
        let file = ThemeFile {
            format_version: THEME_FORMAT_VERSION,
            name: theme.name.clone(),
            base: theme.base,
            colors: theme.colors.clone(),
        };
        std::fs::create_dir_all(&self.dir)?;
        let mut json = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
        json.push(b'\n');
        atomic::write_atomic(&self.path(&theme.id), &json)?;
        Ok(theme)
    }

    /// Removes the copy of an imported theme.
    pub fn remove(&self, id: &str) -> Result<(), ThemeError> {
        if !is_theme_id(id) {
            return Err(ThemeError::NotFound);
        }
        match std::fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(ThemeError::NotFound),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dark design of section 11 as a theme file.
    fn dark_file() -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1,
            "name": "Nacht",
            "base": "dark",
            "colors": {
                "surface": "#1e1e1e", "sidebar": "#252526", "text": "#D4D4D4",
                "textMuted": "#A0A4AA", "accent": "#6C9BFF", "error": "#F48771",
                "success": "#4EC97A", "clusterBlue": "#6C9BFF", "clusterGreen": "#4EC97A",
                "clusterViolet": "#B58CFF", "clusterOrange": "#F0A050", "clusterGrey": "#9AA0A6"
            }
        })
    }

    fn store() -> (ThemeStore, tempfile::TempDir) {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        (ThemeStore::new(dir.path()), dir)
    }

    #[test]
    fn the_section_11_designs_pass() {
        let (name, base, colors) = parse(&dark_file().to_string()).unwrap();
        assert_eq!(name, "Nacht");
        assert_eq!(base, ThemeBase::Dark);
        assert_eq!(colors.surface, "#1E1E1E", "normalised to upper case");
    }

    #[test]
    fn contrast_matches_the_wcag_reference() {
        assert!((contrast_ratio("#767676", "#FFFFFF") - 4.54).abs() < 0.01);
        assert!((contrast_ratio("#000000", "#FFFFFF") - 21.0).abs() < 1e-9);
        assert!((contrast_ratio("#FFFFFF", "#FFFFFF") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn invalid_files_name_the_problem() {
        let cases: Vec<(serde_json::Value, &str)> = vec![
            (serde_json::json!({ "formatVersion": 2 }), "missing field"),
            ({ let mut f = dark_file(); f["formatVersion"] = 2.into(); f }, "formatVersion"),
            ({ let mut f = dark_file(); f["name"] = " ".into(); f }, "name"),
            ({ let mut f = dark_file(); f["name"] = "x".repeat(61).into(); f }, "name"),
            ({ let mut f = dark_file(); f["colors"]["accent"] = "blue".into(); f }, "colors.accent"),
            ({ let mut f = dark_file(); f["colors"]["text"] = "#FFF".into(); f }, "colors.text"),
            ({ let mut f = dark_file(); f["colors"]["surface"] = "url(x)".into(); f }, "colors.surface"),
            ({ let mut f = dark_file(); f["colors"]["font"] = "#FFFFFF".into(); f }, "unknown field"),
            ({ let mut f = dark_file(); f["script"] = "alert(1)".into(); f }, "unknown field"),
            ({ let mut f = dark_file(); f["base"] = "neon".into(); f }, "unknown variant"),
        ];
        for (file, expected) in cases {
            match parse(&file.to_string()) {
                Err(ThemeError::Invalid { field }) => {
                    assert!(field.contains(expected), "{field} should contain {expected}");
                }
                other => panic!("{file}: expected Invalid, got {other:?}"),
            }
        }
        assert!(matches!(parse("not json"), Err(ThemeError::Invalid { .. })));
        assert!(matches!(
            parse(&" ".repeat(MAX_THEME_BYTES + 1)),
            Err(ThemeError::Invalid { field }) if field == "size"
        ));
    }

    #[test]
    fn low_contrast_is_rejected_with_the_pair() {
        let mut file = dark_file();
        file["colors"]["textMuted"] = "#555555".into();
        match parse(&file.to_string()) {
            Err(ThemeError::Contrast { pair, ratio }) => {
                assert_eq!(pair, "textMuted/surface");
                assert!(ratio < 4.5);
            }
            other => panic!("expected Contrast, got {other:?}"),
        }
        let mut file = dark_file();
        file["colors"]["clusterGrey"] = "#3A3A3A".into();
        assert!(matches!(
            parse(&file.to_string()),
            Err(ThemeError::Contrast { pair, .. }) if pair == "clusterGrey/surface"
        ));
    }

    #[test]
    fn themes_are_stored_listed_and_removed() {
        let (store, _dir) = store();
        assert!(store.list().is_empty());
        let id = "0123456789abcdef0123456789abcdef".to_owned();
        let theme = store.import(&dark_file().to_string(), id.clone()).unwrap();
        assert_eq!(theme.name, "Nacht");
        let listed = store.list();
        assert_eq!(listed, vec![theme]);

        // A file edited by hand into something invalid is skipped, not deleted.
        let broken = store.path("fedcba9876543210fedcba9876543210");
        std::fs::write(&broken, "{}").unwrap();
        std::fs::write(store.dir.join("notes.txt"), "x").unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(broken.exists());

        store.remove(&id).unwrap();
        assert!(store.list().is_empty());
        assert!(matches!(store.remove(&id), Err(ThemeError::NotFound)));
        assert!(matches!(store.remove("../settings"), Err(ThemeError::NotFound)));
    }
}
