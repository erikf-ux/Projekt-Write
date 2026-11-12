//! Language check (section 14, phase 12).
//!
//! The parts here do not depend on a checking engine: sentence
//! segmentation with exception lists per language, and the uniqueness rule
//! that decides whether a misspelt word may be corrected automatically.
//! The engines themselves (`spellbook` per ADR 0023, LanguageTool as an
//! add-on) come behind the `LanguageEngine` interface.

pub mod autocorrect;
pub mod sentences;
