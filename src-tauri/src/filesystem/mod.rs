//! File system access for the library (section 9).
//!
//! * [`atomic`]: crash-safe file replacement.
//! * [`location`]: recognises sync folders and network drives.
//!
//! Paths are checked with [`crate::security::paths`] before they reach these
//! functions.

pub mod atomic;
pub mod location;

#[cfg(windows)]
mod windows;
