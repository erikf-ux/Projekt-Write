//! Opening web links in the system browser (section 12: "Weblinks öffnen
//! mit Ctrl + Klick im Systembrowser. Erlaubt sind nur `http`, `https` und
//! `mailto`.").
//!
//! The address goes to the system as one argument, never through a shell:
//! `ShellExecuteW` on Windows (`filesystem/windows.rs`, the crate's only
//! `unsafe` code), `xdg-open` on Linux, `open` on macOS. The app itself loads
//! nothing.

use crate::error::{CommandError, ErrorCode};

/// Longest address that can be opened.
pub const MAX_LINK_CHARS: usize = 2048;

/// Checks that `url` is an http, https or mailto address without spaces or
/// control characters.
pub fn check(url: &str) -> Result<(), CommandError> {
    if url.is_empty() || url.chars().count() > MAX_LINK_CHARS {
        return Err(CommandError::invalid_input("url", "length"));
    }
    let lower = url.to_ascii_lowercase();
    let rest = ["https://", "http://", "mailto:"]
        .iter()
        .find_map(|scheme| lower.strip_prefix(scheme));
    let allowed = rest.is_some_and(|rest| !rest.is_empty())
        && !url.chars().any(|c| c.is_whitespace() || c.is_control());
    if allowed {
        Ok(())
    } else {
        Err(CommandError::new(ErrorCode::LinkNotAllowed))
    }
}

/// Opens a checked address in the system browser or mail program.
pub fn open(url: &str) -> Result<(), CommandError> {
    check(url)?;
    platform_open(url).map_err(|error| {
        log::warn!("link could not be opened: {error}");
        CommandError::new(ErrorCode::LinkOpenFailed)
    })
}

#[cfg(windows)]
fn platform_open(url: &str) -> std::io::Result<()> {
    crate::filesystem::windows::shell_open(url)
}

#[cfg(not(windows))]
fn platform_open(url: &str) -> std::io::Result<()> {
    use std::process::{Command, Stdio};

    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap the helper when it exits; it hands the address to the browser.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_addresses_are_allowed() {
        for url in [
            "https://example.org",
            "http://example.org/pfad?x=1&y=2#teil",
            "HTTPS://EXAMPLE.ORG",
            "mailto:anna@example.org",
            "https://de.wikipedia.org/wiki/Klammer_(Zeichen)",
        ] {
            assert_eq!(check(url), Ok(()), "{url}");
        }
    }

    #[test]
    fn other_schemes_and_odd_addresses_are_refused() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,x",
            "ftp://example.org",
            "pwrite://dokument",
            "kapitel-2.md",
            "https://",
            "mailto:",
            "https://example.org/mit leerzeichen",
            "https://example.org/\nzeile",
            "-https://example.org",
        ] {
            assert_eq!(
                check(url).map_err(|error| error.code),
                Err(ErrorCode::LinkNotAllowed),
                "{url}"
            );
        }
        let long = format!("https://example.org/{}", "a".repeat(MAX_LINK_CHARS));
        assert_eq!(
            check(&long).map_err(|error| error.code),
            Err(ErrorCode::InvalidInput)
        );
        assert_eq!(
            check("").map_err(|error| error.code),
            Err(ErrorCode::InvalidInput)
        );
    }
}
