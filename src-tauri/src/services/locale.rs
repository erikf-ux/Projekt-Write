//! Language of the interface (section 11: German and English, "standardmäßig
//! passend zum System").
//!
//! Rust decides the language once for both the native menu and the webview,
//! so the two never disagree.

use serde::Serialize;
use ts_rs::TS;

use crate::services::settings::ui::LanguageSetting;

/// A language the interface is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Locale {
    De,
    En,
}

impl Locale {
    /// The language a setting stands for, `system` being the language of the
    /// operating system.
    pub fn effective(setting: LanguageSetting, system: Locale) -> Locale {
        match setting {
            LanguageSetting::System => system,
            LanguageSetting::De => Locale::De,
            LanguageSetting::En => Locale::En,
        }
    }

    /// German for a German language tag (`de`, `de_DE.UTF-8`, `de-AT`),
    /// otherwise English.
    fn from_tag(tag: &str) -> Option<Locale> {
        let primary = tag
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "" | "c" | "posix" => None,
            "de" => Some(Locale::De),
            _ => Some(Locale::En),
        }
    }
}

/// The language of the operating system.
pub fn system_locale() -> Locale {
    #[cfg(windows)]
    {
        const LANG_GERMAN: u16 = 0x07;
        if crate::filesystem::windows::user_ui_primary_language() == LANG_GERMAN {
            Locale::De
        } else {
            Locale::En
        }
    }
    #[cfg(not(windows))]
    {
        from_environment(|name| std::env::var(name).ok())
    }
}

/// The language from the environment, in the order gettext uses:
/// `LANGUAGE` (a colon-separated list), `LC_ALL`, `LC_MESSAGES`, `LANG`.
/// English if none names a language.
#[cfg_attr(windows, allow(dead_code))]
fn from_environment(var: impl Fn(&str) -> Option<String>) -> Locale {
    for name in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = var(name) else { continue };
        for tag in value.split(':') {
            if let Some(locale) = Locale::from_tag(tag.trim()) {
                return locale;
            }
        }
    }
    Locale::En
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn environment_follows_the_gettext_order() {
        assert_eq!(
            from_environment(env(&[("LANG", "de_DE.UTF-8")])),
            Locale::De
        );
        assert_eq!(
            from_environment(env(&[("LANG", "en_US.UTF-8")])),
            Locale::En
        );
        assert_eq!(
            from_environment(env(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "de_AT.UTF-8")])),
            Locale::De
        );
        assert_eq!(
            from_environment(env(&[("LANG", "de_DE.UTF-8"), ("LANGUAGE", "fr:de")])),
            Locale::En,
            "the first language of LANGUAGE wins, French falls back to English"
        );
        assert_eq!(
            from_environment(env(&[("LANGUAGE", ""), ("LANG", "de_CH")])),
            Locale::De
        );
        assert_eq!(from_environment(env(&[("LANG", "C")])), Locale::En);
        assert_eq!(
            from_environment(env(&[("LC_ALL", "POSIX"), ("LANG", "de_DE")])),
            Locale::De
        );
        assert_eq!(from_environment(env(&[])), Locale::En);
    }

    #[test]
    fn explicit_settings_override_the_system() {
        assert_eq!(
            Locale::effective(LanguageSetting::System, Locale::De),
            Locale::De
        );
        assert_eq!(
            Locale::effective(LanguageSetting::En, Locale::De),
            Locale::En
        );
        assert_eq!(
            Locale::effective(LanguageSetting::De, Locale::En),
            Locale::De
        );
    }
}
