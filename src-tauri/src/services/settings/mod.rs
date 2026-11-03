//! Settings (section 9, "Einstellungen").
//!
//! * [`app`]: app-wide settings in the configuration folder of the system,
//!   needed before a library is open.
//! * [`library`]: library-wide settings in `application_settings`.
//! * [`ui`]: interface settings and window state (part of `settings.json`).

pub mod app;
pub mod library;
pub mod ui;
