//! Files opened from outside the app (section 9, "Dateizuordnung"; ADR 0015,
//! decision 15).
//!
//! A double-click or "Öffnen mit" starts the app with the file as argument
//! (Windows, Linux). If the app runs already, `tauri-plugin-single-instance`
//! hands the arguments of the second start to the running one. Both land in
//! [`OpenQueue`] until the interface takes them (`open_requests_take`), so a
//! file opened while the app starts or while no library is open is not
//! lost. macOS sends `RunEvent::Opened` instead; it rests with macOS.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Most files waiting at once; more are dropped (and logged).
const MAX_QUEUED: usize = 64;

#[derive(Debug, Default)]
pub struct OpenQueue {
    paths: Mutex<VecDeque<PathBuf>>,
}

impl OpenQueue {
    /// Adds files to open; returns whether any were added.
    pub fn push(&self, paths: impl IntoIterator<Item = PathBuf>) -> bool {
        let mut queue = self
            .paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = queue.len();
        for path in paths {
            if queue.len() >= MAX_QUEUED {
                log::warn!("too many files to open at once; the rest is ignored");
                break;
            }
            if !queue.contains(&path) {
                queue.push_back(path);
            }
        }
        queue.len() > before
    }

    /// The waiting files, oldest first; the queue is empty afterwards.
    pub fn take(&self) -> Vec<PathBuf> {
        self.paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect()
    }
}

/// The files among the arguments of a start: everything after the program
/// that is not an option (`-…`), relative paths resolved against `cwd`,
/// `file://` addresses as paths.
pub fn paths_from_args(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter_map(|argument| {
            if argument.to_string_lossy().starts_with('-') {
                return None;
            }
            let path = match argument
                .to_str()
                .and_then(|text| text.strip_prefix("file://"))
            {
                Some(address) => PathBuf::from(file_url_path(address)?),
                None => PathBuf::from(argument),
            };
            if path.as_os_str().is_empty() {
                return None;
            }
            Some(if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            })
        })
        .collect()
}

/// The path of a `file://` address without host (`file:///home/a/b.md`),
/// percent-decoded. Other hosts are not local files.
fn file_url_path(address: &str) -> Option<String> {
    let path = address.strip_prefix("localhost").unwrap_or(address);
    if !path.starts_with('/') {
        return None;
    }
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let text = String::from_utf8(decoded).ok()?;
    // Windows: file:///C:/Texte/a.md
    if cfg!(windows) && text.as_bytes().get(2) == Some(&b':') {
        return Some(text[1..].replace('/', "\\"));
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn arguments_become_paths() {
        let cwd = Path::new(if cfg!(windows) {
            r"C:\Arbeit"
        } else {
            "/home/a/Arbeit"
        });
        let absolute = if cfg!(windows) {
            r"C:\Texte\Brief.md"
        } else {
            "/home/a/Brief.md"
        };
        let paths = paths_from_args(
            args(&["project-write", "--flag", "Kapitel 1.pwrite", absolute, ""]),
            cwd,
        );
        assert_eq!(
            paths,
            [cwd.join("Kapitel 1.pwrite"), PathBuf::from(absolute)]
        );
        assert!(paths_from_args(args(&["project-write"]), cwd).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn file_addresses_are_decoded() {
        let cwd = Path::new("/");
        assert_eq!(
            paths_from_args(
                args(&[
                    "pw",
                    "file:///home/a/Gr%C3%B6%C3%9Fe%20neu.md",
                    "file://server/x.md"
                ]),
                cwd
            ),
            [PathBuf::from("/home/a/Größe neu.md")]
        );
        assert!(paths_from_args(args(&["pw", "file:///a/%ZZ.md"]), cwd).is_empty());
    }

    #[test]
    fn the_queue_keeps_order_and_drops_duplicates() {
        let queue = OpenQueue::default();
        assert!(queue.push([PathBuf::from("a.md"), PathBuf::from("b.md")]));
        assert!(!queue.push([PathBuf::from("a.md")]));
        assert_eq!(queue.take(), [PathBuf::from("a.md"), PathBuf::from("b.md")]);
        assert!(queue.take().is_empty());
        assert!(queue.push((0..100).map(|n| PathBuf::from(format!("{n}.md")))));
        assert_eq!(queue.take().len(), MAX_QUEUED);
    }
}
