//! The registry of checking engines (section 14; ADR 0027, decision 1).
//!
//! A check runs the registered engines in registration order; each entry
//! decides from the check whether it takes part and adds its findings:
//!
//! 1. `spelling`: Hunspell dictionaries through `spellbook` (ADR 0023),
//!    when the language has a dictionary;
//! 2. `rules`: the simple rules (double word, double space, sentence
//!    start);
//! 3. `languagetool`: the LanguageTool add-on (ADR 0026), only while it
//!    runs. A place an earlier engine already found is not marked twice.
//!
//! The entries hold no state: the dictionaries belong to the
//! [`LanguageService`], the running add-ons are handed to each check
//! ([`AddOns`]).

use super::engine::{LanguageEngine, OwnWords, RawFinding};
use super::{CheckParts, LanguageService, rules};
use crate::services::registry::RegistryEntry;

/// Ids of the built-in engines.
pub const SPELLING: &str = "spelling";
pub const RULES: &str = "rules";
pub const LANGUAGETOOL: &str = "languagetool";

/// The add-on engines that run now, by registry id.
#[derive(Default)]
pub struct AddOns<'a> {
    running: Vec<(&'static str, &'a dyn LanguageEngine)>,
}

impl<'a> AddOns<'a> {
    /// No add-on runs.
    pub fn none() -> Self {
        Self::default()
    }

    /// The add-on with the registry id `id` runs as `engine`.
    pub fn with(mut self, id: &'static str, engine: &'a dyn LanguageEngine) -> Self {
        self.running.push((id, engine));
        self
    }

    pub fn get(&self, id: &str) -> Option<&'a dyn LanguageEngine> {
        self.running
            .iter()
            .find(|(running, _)| *running == id)
            .map(|(_, engine)| *engine)
    }
}

/// One check of one text, passed through the engines.
pub struct CheckRun<'a> {
    pub(super) service: &'a LanguageService,
    pub(super) text: &'a str,
    pub(super) language: &'a str,
    /// The other languages of the document's profile.
    pub(super) also: &'a [String],
    pub(super) own: &'a OwnWords,
    pub(super) parts: CheckParts,
    pub(super) has_dictionary: bool,
    pub(super) add_ons: &'a AddOns<'a>,
    /// What the engines found so far.
    pub(super) found: Vec<RawFinding>,
}

/// A registered checking engine.
pub trait CheckingEngine: RegistryEntry + Send + Sync {
    /// An add-on installed by the user (shown as such in the settings),
    /// not part of the app.
    fn add_on(&self) -> bool;

    /// Adds the findings of this engine to `run`, if it takes part.
    fn run(&self, run: &mut CheckRun<'_>);
}

/// The spelling check; a word correct in another language of the profile
/// is not flagged.
struct Spelling;

impl RegistryEntry for Spelling {
    fn id(&self) -> &'static str {
        SPELLING
    }
}

impl CheckingEngine for Spelling {
    fn add_on(&self) -> bool {
        false
    }

    fn run(&self, run: &mut CheckRun<'_>) {
        if !run.parts.spelling || !run.has_dictionary {
            return;
        }
        let service = run.service;
        let (text, language, also, own) = (run.text, run.language, run.also, run.own);
        match service.spelling.check(text, language, own) {
            Ok(found) => run.found.extend(found.into_iter().filter(|finding| {
                let word = &text[finding.start..finding.end];
                !also
                    .iter()
                    .filter(|other| other.as_str() != language)
                    .any(|other| service.is_correct(word, other, own).unwrap_or(false))
            })),
            Err(error) => log::warn!("spelling check failed: {error}"),
        }
    }
}

/// The simple rules.
struct Rules;

impl RegistryEntry for Rules {
    fn id(&self) -> &'static str {
        RULES
    }
}

impl CheckingEngine for Rules {
    fn add_on(&self) -> bool {
        false
    }

    fn run(&self, run: &mut CheckRun<'_>) {
        if run.parts.rules {
            run.found.extend(rules::check(run.text, run.language));
        }
    }
}

/// The LanguageTool add-on: grammar, punctuation, context.
struct LanguageToolAddOn;

impl RegistryEntry for LanguageToolAddOn {
    fn id(&self) -> &'static str {
        LANGUAGETOOL
    }
}

impl CheckingEngine for LanguageToolAddOn {
    fn add_on(&self) -> bool {
        true
    }

    fn run(&self, run: &mut CheckRun<'_>) {
        let Some(engine) = run.add_ons.get(self.id()) else {
            return;
        };
        match engine.check(run.text, run.language, run.own) {
            Ok(found) => {
                let known: Vec<(usize, usize)> =
                    run.found.iter().map(|f| (f.start, f.end)).collect();
                run.found.extend(found.into_iter().filter(|f| {
                    !known
                        .iter()
                        .any(|&(start, end)| f.start < end && start < f.end)
                }));
            }
            Err(error) => log::warn!("{} failed: {error}", engine.name()),
        }
    }
}

/// The built-in checking engines, in the order they run.
pub fn builtin() -> Vec<Box<dyn CheckingEngine>> {
    vec![
        Box::new(Spelling),
        Box::new(Rules),
        Box::new(LanguageToolAddOn),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_languagetool_is_an_add_on() {
        let add_ons: Vec<(&str, bool)> = builtin()
            .iter()
            .map(|engine| (engine.id(), engine.add_on()))
            .collect();
        assert_eq!(
            add_ons,
            [(SPELLING, false), (RULES, false), (LANGUAGETOOL, true)]
        );
    }

    #[test]
    fn add_ons_are_found_by_id() {
        struct Silent;
        impl LanguageEngine for Silent {
            fn name(&self) -> &'static str {
                "silent"
            }
            fn check(
                &self,
                _text: &str,
                _language: &str,
                _own: &OwnWords,
            ) -> Result<Vec<RawFinding>, super::super::engine::EngineError> {
                Ok(Vec::new())
            }
        }
        let add_ons = AddOns::none().with(LANGUAGETOOL, &Silent);
        assert_eq!(add_ons.get(LANGUAGETOOL).map(|e| e.name()), Some("silent"));
        assert!(add_ons.get(SPELLING).is_none());
        assert!(AddOns::none().get(LANGUAGETOOL).is_none());
    }
}
