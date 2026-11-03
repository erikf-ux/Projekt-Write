//! Path checks for every file Rust reads or writes inside a library
//! (section 20): "Pfade werden aufgelöst und gegen den Bibliotheksordner
//! geprüft, symbolische Links werden abgelehnt."
//!
//! * Paths inside a library are always *relative* and written with `/`. They
//!   come from the application (UUID file names, fixed folder names), never
//!   from the frontend.
//! * Every component is checked against a strict rule set that is valid on
//!   Windows, macOS and Linux alike, so a library stays portable.
//! * Every existing prefix of the resolved path is inspected with
//!   `symlink_metadata`. Symbolic links and Windows junctions are rejected,
//!   wherever they appear. The library root itself is canonicalised once when
//!   the library is opened, so a root reached through a link the user chose is
//!   allowed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Longest accepted component in bytes (common file system limit).
const MAX_COMPONENT_LEN: usize = 255;

/// Device names Windows reserves in every folder, with or without extension.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM0", "COM1", "COM2", "COM3", "COM4",
    "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3",
    "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("empty path")]
    Empty,
    #[error("path is not relative to the library: {relative:?}")]
    NotRelative { relative: String },
    #[error("invalid path component {component:?}: {reason}")]
    InvalidComponent {
        component: String,
        reason: &'static str,
    },
    #[error("symbolic link or junction at {relative:?}")]
    SymbolicLink { relative: String },
    #[error("{relative:?} is not a directory")]
    NotADirectory { relative: String },
    #[error("{relative:?} is not a regular file")]
    NotAFile { relative: String },
    #[error("path {relative:?} leads outside the library")]
    OutsideRoot { relative: String },
    #[error("library folder is not usable: {source}")]
    Root {
        #[source]
        source: io::Error,
    },
    #[error("I/O error at {relative:?}: {source}")]
    Io {
        relative: String,
        #[source]
        source: io::Error,
    },
}

impl PathError {
    /// The library-relative path the error refers to, if any.
    pub fn relative(&self) -> Option<&str> {
        match self {
            PathError::NotRelative { relative }
            | PathError::SymbolicLink { relative }
            | PathError::NotADirectory { relative }
            | PathError::NotAFile { relative }
            | PathError::OutsideRoot { relative }
            | PathError::Io { relative, .. } => Some(relative),
            PathError::InvalidComponent { component, .. } => Some(component),
            PathError::Empty | PathError::Root { .. } => None,
        }
    }
}

/// Checks a single file or folder name.
pub fn validate_component(component: &str) -> Result<(), PathError> {
    let invalid = |reason| {
        Err(PathError::InvalidComponent {
            component: component.to_owned(),
            reason,
        })
    };
    if component.is_empty() {
        return invalid("empty");
    }
    if component == "." || component == ".." {
        return invalid("relative component");
    }
    if component.len() > MAX_COMPONENT_LEN {
        return invalid("too long");
    }
    if let Some(c) = component.chars().find(|&c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return if c.is_control() {
            invalid("control character")
        } else {
            invalid("reserved character")
        };
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return invalid("trailing dot or space");
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches(' ');
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem) || *reserved == stem.to_uppercase())
    {
        return invalid("reserved device name");
    }
    Ok(())
}

/// Splits and validates a library-relative path such as `documents/text/x.txt`.
pub fn split_relative(relative: &str) -> Result<Vec<&str>, PathError> {
    if relative.is_empty() {
        return Err(PathError::Empty);
    }
    if relative.starts_with('/') || relative.starts_with('\\') {
        return Err(PathError::NotRelative {
            relative: relative.to_owned(),
        });
    }
    let components: Vec<&str> = relative.split('/').collect();
    for component in &components {
        validate_component(component)?;
    }
    Ok(components)
}

/// A library folder: the root against which all relative paths are resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeRoot {
    root: PathBuf,
}

impl SafeRoot {
    /// Canonicalises `path` (resolving links in the path the user chose) and
    /// checks that it is an existing directory.
    pub fn new(path: &Path) -> Result<Self, PathError> {
        let canonical = canonical_dir(path).map_err(|source| PathError::Root { source })?;
        Ok(Self { root: canonical })
    }

    /// The canonical absolute path of the root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Resolves `relative` below the root.
    ///
    /// Every component must pass [`validate_component`]. Every prefix that
    /// exists must not be a symbolic link or junction, and every prefix but the
    /// last must be a directory. The final component need not exist.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, PathError> {
        let components = split_relative(relative)?;
        let mut path = self.root.clone();
        for (index, component) in components.iter().enumerate() {
            path.push(component);
            let shown = || components[..=index].join("/");
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(PathError::SymbolicLink { relative: shown() });
                    }
                    if index + 1 < components.len() && !metadata.is_dir() {
                        return Err(PathError::NotADirectory { relative: shown() });
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    // Nothing below a missing component can exist either.
                    path.extend(&components[index + 1..]);
                    break;
                }
                Err(source) => {
                    return Err(PathError::Io {
                        relative: shown(),
                        source,
                    });
                }
            }
        }
        // Defence in depth: the joined path must still lie below the root.
        if !path.starts_with(&self.root) {
            return Err(PathError::OutsideRoot {
                relative: relative.to_owned(),
            });
        }
        Ok(path)
    }

    /// Resolves `relative` and requires an existing regular file.
    pub fn resolve_file(&self, relative: &str) -> Result<PathBuf, PathError> {
        let path = self.resolve(relative)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => Ok(path),
            Ok(_) => Err(PathError::NotAFile {
                relative: relative.to_owned(),
            }),
            Err(source) => Err(PathError::Io {
                relative: relative.to_owned(),
                source,
            }),
        }
    }

    /// Creates `relative` and all missing parents as directories, refusing to
    /// pass through links. Existing directories are accepted.
    pub fn create_dir_all(&self, relative: &str) -> Result<PathBuf, PathError> {
        let components = split_relative(relative)?;
        let mut path = self.root.clone();
        for (index, component) in components.iter().enumerate() {
            path.push(component);
            let shown = || components[..=index].join("/");
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(PathError::SymbolicLink { relative: shown() });
                }
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => return Err(PathError::NotADirectory { relative: shown() }),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if let Err(source) = create_private_dir(&path)
                        && source.kind() != io::ErrorKind::AlreadyExists
                    {
                        return Err(PathError::Io {
                            relative: shown(),
                            source,
                        });
                    }
                    // A concurrent creation (or our own) must have produced a
                    // real directory, not a link.
                    match fs::symlink_metadata(&path) {
                        Ok(metadata) if metadata.file_type().is_symlink() => {
                            return Err(PathError::SymbolicLink { relative: shown() });
                        }
                        Ok(metadata) if metadata.is_dir() => {}
                        Ok(_) => return Err(PathError::NotADirectory { relative: shown() }),
                        Err(source) => {
                            return Err(PathError::Io {
                                relative: shown(),
                                source,
                            });
                        }
                    }
                }
                Err(source) => {
                    return Err(PathError::Io {
                        relative: shown(),
                        source,
                    });
                }
            }
        }
        Ok(path)
    }
}

/// Creates one directory, readable only by the current user on Unix.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::DirBuilder::new().create(path)
    }
}

/// Canonicalises an existing directory and returns the plainest equivalent
/// form (on Windows without the `\\?\` prefix where that is safe).
pub fn canonical_dir(path: &Path) -> io::Result<PathBuf> {
    let canonical = simplify(&fs::canonicalize(path)?);
    if !fs::metadata(&canonical)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "not a directory",
        ));
    }
    Ok(canonical)
}

/// Canonicalises a path whose last components may not exist yet: the nearest
/// existing ancestor is canonicalised and the missing components are appended
/// after validation. Returns `None` if no ancestor exists or a missing
/// component is not a valid name.
pub fn canonical_prospective(path: &Path) -> Option<PathBuf> {
    let mut missing: Vec<String> = Vec::new();
    let mut current = path;
    loop {
        if let Ok(canonical) = fs::canonicalize(current) {
            let mut result = simplify(&canonical);
            for component in missing.iter().rev() {
                validate_component(component).ok()?;
                result.push(component);
            }
            return Some(result);
        }
        let name = current.file_name()?.to_str()?;
        missing.push(name.to_owned());
        current = current.parent()?;
    }
}

/// Removes the Windows verbatim prefix (`\\?\C:\…`, `\\?\UNC\…`) when the
/// remaining path means exactly the same thing. Other platforms: unchanged.
pub fn simplify(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        windows_simplify(path).unwrap_or_else(|| path.to_path_buf())
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn windows_simplify(path: &Path) -> Option<PathBuf> {
    // Paths near MAX_PATH keep the prefix: without it Win32 would reject them.
    const SAFE_LEN: usize = 240;
    let text = path.to_str()?;
    let (plain, check) = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        (format!(r"\\{rest}"), rest)
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || &bytes[1..3] != b":\\" {
            return None;
        }
        (rest.to_owned(), &rest[3..])
    } else {
        return Some(path.to_path_buf());
    };
    if plain.len() > SAFE_LEN {
        return None;
    }
    // Win32 would silently change names with trailing dots or spaces and treat
    // device names specially; such paths keep the verbatim form.
    let all_plain = check
        .split('\\')
        .filter(|c| !c.is_empty())
        .all(|c| validate_component(c).is_ok());
    all_plain.then(|| PathBuf::from(plain))
}

/// The path as the user should see it (text only, never sent back).
pub fn display_path(path: &Path) -> String {
    simplify(path).to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_names() {
        for name in [
            "documents",
            "3f0c2d55-9a33-4c39-9a0e-27b1c84f0f6d.md",
            "projectwrite.db",
            ".hidden",
            "Übersicht – März 2026",
            "a b",
            "CONSOLE",
            "COM10",
            "nul-terminated",
        ] {
            validate_component(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn rejects_dangerous_names() {
        for (name, reason) in [
            ("", "empty"),
            (".", "relative component"),
            ("..", "relative component"),
            ("a/b", "reserved character"),
            ("a\\b", "reserved character"),
            ("C:", "reserved character"),
            ("file:stream", "reserved character"),
            ("a*", "reserved character"),
            ("what?", "reserved character"),
            ("<x>", "reserved character"),
            ("a|b", "reserved character"),
            ("quote\"", "reserved character"),
            ("tab\there", "control character"),
            ("nul\0byte", "control character"),
            ("trailing.", "trailing dot or space"),
            ("trailing ", "trailing dot or space"),
            ("CON", "reserved device name"),
            ("con", "reserved device name"),
            ("nul.txt", "reserved device name"),
            ("Lpt1.md", "reserved device name"),
            ("COM9", "reserved device name"),
            ("aux .txt", "reserved device name"),
        ] {
            match validate_component(name) {
                Err(PathError::InvalidComponent { reason: r, .. }) => {
                    assert_eq!(r, reason, "{name:?}");
                }
                other => panic!("{name:?} was not rejected: {other:?}"),
            }
        }
        let long = "x".repeat(MAX_COMPONENT_LEN + 1);
        assert!(validate_component(&long).is_err());
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        for relative in [
            "../outside",
            "documents/../../outside",
            "documents/./x",
            "/etc/passwd",
            "\\windows",
            "C:/Windows",
            "documents//x",
            "documents/",
            "",
        ] {
            assert!(split_relative(relative).is_err(), "{relative:?}");
        }
        assert_eq!(
            split_relative("documents/text/a.txt").unwrap(),
            ["documents", "text", "a.txt"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn simplifies_windows_verbatim_paths() {
        assert_eq!(
            simplify(Path::new(r"\\?\C:\Users\x\Documents")),
            PathBuf::from(r"C:\Users\x\Documents")
        );
        assert_eq!(
            simplify(Path::new(r"\\?\UNC\server\share\lib")),
            PathBuf::from(r"\\server\share\lib")
        );
        let odd = Path::new(r"\\?\C:\Users\x\trailing.\lib");
        assert_eq!(simplify(odd), odd.to_path_buf());
        let device = Path::new(r"\\?\C:\Users\x\con\lib");
        assert_eq!(simplify(device), device.to_path_buf());
    }
}
