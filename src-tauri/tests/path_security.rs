//! Acceptance test of Phase 2 (section 25): "Pfadangriffe wie `..` und
//! symbolische Links werden abgelehnt."
//!
//! Uses real links and, on Windows, junctions. Symbolic links on Windows need
//! administrator rights or developer mode; the CI runners have them. A missing
//! right fails the test instead of skipping it.

mod common;

use std::fs;

use project_write_lib::filesystem::atomic;
use project_write_lib::security::paths::{PathError, SafeRoot};

/// A library root with `documents/text/` and a folder outside of it.
struct Fixture {
    _dir: tempfile::TempDir,
    root: SafeRoot,
    outside: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = common::temp_dir("pw-paths-");
    let library = dir.path().join("library");
    fs::create_dir_all(library.join("documents").join("text")).unwrap();
    fs::write(
        library.join("documents").join("text").join("a.txt"),
        b"inside",
    )
    .unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"outside").unwrap();
    Fixture {
        root: SafeRoot::new(&library).unwrap(),
        outside,
        _dir: dir,
    }
}

#[test]
fn traversal_and_foreign_roots_are_rejected() {
    let f = fixture();
    for attack in [
        "..",
        "../outside/secret.txt",
        "documents/../../outside/secret.txt",
        "documents/text/../../../outside",
        "./documents",
        "/etc/passwd",
        "\\Windows\\system.ini",
        "C:/Windows/system.ini",
        "C:\\Windows",
        "documents\\text\\a.txt",
        "documents/text/a.txt:stream",
        "documents//text",
        "documents/text/",
        "documents/CON",
        "documents/nul.txt",
        "documents/text/a.txt.",
        "",
    ] {
        let result = f.root.resolve(attack);
        assert!(result.is_err(), "{attack:?} resolved to {result:?}");
        assert!(f.root.create_dir_all(attack).is_err(), "{attack:?}");
    }
    assert_eq!(
        f.root.resolve("documents/text/a.txt").unwrap(),
        f.root.path().join("documents").join("text").join("a.txt")
    );
}

/// Checks that every way into `link` is refused.
fn assert_link_rejected(f: &Fixture, link: &str) {
    for relative in [link.to_owned(), format!("{link}/secret.txt")] {
        match f.root.resolve(&relative) {
            Err(PathError::SymbolicLink { relative: at }) => assert_eq!(at, link),
            other => panic!("{relative}: expected SymbolicLink, got {other:?}"),
        }
    }
    assert!(matches!(
        f.root.create_dir_all(&format!("{link}/new")),
        Err(PathError::SymbolicLink { .. })
    ));
    assert!(f.root.resolve_file(&format!("{link}/secret.txt")).is_err());
    // Nothing was created behind the link.
    assert!(!f.outside.join("new").exists());
}

#[test]
fn symbolic_link_to_a_folder_outside_is_rejected() {
    let f = fixture();
    common::symlink_dir(&f.outside, &f.root.path().join("documents").join("evil"))
        .expect("create directory symlink (Windows: needs admin rights or developer mode)");
    assert_link_rejected(&f, "documents/evil");
}

#[test]
fn symbolic_link_to_a_file_is_rejected() {
    let f = fixture();
    let link = f.root.path().join("documents").join("text").join("b.txt");
    common::symlink_file(&f.outside.join("secret.txt"), &link)
        .expect("create file symlink (Windows: needs admin rights or developer mode)");
    assert!(matches!(
        f.root.resolve("documents/text/b.txt"),
        Err(PathError::SymbolicLink { .. })
    ));
    assert!(f.root.resolve_file("documents/text/b.txt").is_err());
    // Even a caller that skips the check cannot read through the link on Unix.
    #[cfg(unix)]
    assert!(atomic::open_read_no_follow(&link).is_err());
    // Writing atomically replaces the link itself, never the file behind it.
    atomic::write_atomic(&link, b"replaced").unwrap();
    assert_eq!(fs::read(f.outside.join("secret.txt")).unwrap(), b"outside");
}

#[cfg(windows)]
#[test]
fn windows_junction_is_rejected() {
    let f = fixture();
    common::junction(
        &f.outside,
        &f.root.path().join("documents").join("junction"),
    )
    .expect("create junction");
    assert_link_rejected(&f, "documents/junction");
}

#[test]
fn a_file_in_the_middle_of_a_path_is_rejected() {
    let f = fixture();
    assert!(matches!(
        f.root.resolve("documents/text/a.txt/x"),
        Err(PathError::NotADirectory { .. })
    ));
    assert!(f.root.create_dir_all("documents/text/a.txt/x").is_err());
}

#[test]
fn root_reached_through_a_link_is_canonicalised() {
    let f = fixture();
    let alias = f.outside.parent().unwrap().join("alias");
    common::symlink_dir(f.root.path(), &alias)
        .expect("create directory symlink (Windows: needs admin rights or developer mode)");
    let via_alias = SafeRoot::new(&alias).unwrap();
    assert_eq!(via_alias.path(), f.root.path());
    assert!(via_alias.resolve("documents/text/a.txt").is_ok());
}

#[test]
fn create_dir_all_builds_missing_folders_only() {
    let f = fixture();
    let created = f.root.create_dir_all("media/images").unwrap();
    assert!(created.is_dir());
    // Existing folders are accepted.
    f.root.create_dir_all("documents/text").unwrap();
    f.root.create_dir_all("media/images").unwrap();
}
