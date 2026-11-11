//! File names and links for exports (ADR 0021, decisions 3 and 4).
//!
//! * Names come from titles and original names and must be valid on all
//!   three systems: no `< > : " / \ | ? *` or control characters, no
//!   trailing dot or space, none of the reserved Windows names (`CON`,
//!   `NUL`, `COM1` …), at most 120 characters. Repeats get ` (2)`, ` (3)` …
//! * Links in the exported Markdown: `pwmedia://<id>` becomes the relative
//!   path of the copied file (`#page=<n>` stays), `pwrite://<id>` the
//!   relative path of the exported document or, if there is none, the link
//!   becomes its text. Paths are percent-encoded, so spaces and umlauts
//!   work in Markdown and HTML alike.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::services::documents::{INTERNAL_LINK_PREFIX, MEDIA_PREFIX};

/// Longest part of an exported path, in characters.
pub const MAX_NAME_CHARS: usize = 120;

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A file or folder name from `title`, without extension; `fallback` when
/// nothing usable is left.
pub fn safe_name(title: &str, fallback: &str) -> String {
    let mut name: String = title
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name.chars().take(MAX_NAME_CHARS).collect();
    while name.ends_with('.') || name.ends_with(' ') {
        name.pop();
    }
    while name.starts_with('.') || name.starts_with(' ') {
        name.remove(0);
    }
    if name.is_empty() {
        name = fallback.to_owned();
    }
    let stem = name.split('.').next().unwrap_or_default().to_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        name.insert(0, '_');
    }
    name
}

/// Hands out names that are unique within one folder (without regard to
/// case, as on Windows and macOS).
#[derive(Debug, Default)]
pub struct UniqueNames {
    taken: BTreeSet<String>,
}

impl UniqueNames {
    /// `stem` with `extension` (without dot; empty for folders), numbered if
    /// taken.
    pub fn take(&mut self, stem: &str, extension: &str) -> String {
        let join = |stem: &str| {
            if extension.is_empty() {
                stem.to_owned()
            } else {
                format!("{stem}.{extension}")
            }
        };
        let mut candidate = join(stem);
        let mut number = 2;
        while !self.taken.insert(candidate.to_lowercase()) {
            candidate = join(&format!("{stem} ({number})"));
            number += 1;
        }
        candidate
    }
}

/// Percent-encodes a relative path for a Markdown link; `/` stays.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The relative path from the folder `from` (`a/b`, empty for the top) to
/// `to` (`a/c/d.md`), with `/`.
pub fn relative_path(from: &str, to: &str) -> String {
    let from: Vec<&str> = from.split('/').filter(|part| !part.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|part| !part.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = std::iter::repeat_n("..", from.len() - common).collect();
    parts.extend(&to[common..]);
    parts.join("/")
}

/// What links point to in an exported Markdown file.
#[derive(Debug, Default)]
pub struct LinkTargets {
    /// Document id → relative path from the folder of the file.
    pub documents: BTreeMap<Uuid, String>,
    /// Medium id → relative path from the folder of the file.
    pub media: BTreeMap<Uuid, String>,
}

fn id_at(text: &str) -> Option<Uuid> {
    let candidate = text.get(..36)?;
    let id = Uuid::try_parse(candidate).ok()?;
    (id.hyphenated().to_string() == candidate).then_some(id)
}

/// Rewrites the links of an exported Markdown text (see the module notes).
pub fn rewrite_links(markdown: &str, targets: &LinkTargets) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest
        .find(MEDIA_PREFIX)
        .into_iter()
        .chain(rest.find(INTERNAL_LINK_PREFIX))
        .min()
    {
        let (before, from) = rest.split_at(start);
        if let Some(after) = from.strip_prefix(MEDIA_PREFIX)
            && let Some(id) = id_at(after)
        {
            out.push_str(before);
            match targets.media.get(&id) {
                Some(path) => out.push_str(&encode_path(path)),
                // A medium that is gone: the address stays as it was.
                None => out.push_str(&from[..MEDIA_PREFIX.len() + 36]),
            }
            rest = &after[36..];
            continue;
        }
        if let Some(after) = from.strip_prefix(INTERNAL_LINK_PREFIX)
            && let Some(id) = id_at(after)
        {
            let tail = &after[36..];
            match targets.documents.get(&id) {
                Some(path) => {
                    out.push_str(before);
                    out.push_str(&encode_path(path));
                    rest = tail;
                }
                None => {
                    // `[text](pwrite://id)` becomes `text`, `<pwrite://id>`
                    // goes; anything else loses the address only.
                    if let Some(open) = before.strip_suffix("](").map(str::len)
                        && let Some(bracket) = before[..open].rfind('[')
                        && let Some(close) = tail.find(')')
                    {
                        out.push_str(&before[..bracket]);
                        out.push_str(&before[bracket + 1..open]);
                        rest = &tail[close + 1..];
                    } else if let (Some(head), Some(after)) =
                        (before.strip_suffix('<'), tail.strip_prefix('>'))
                    {
                        out.push_str(head);
                        rest = after;
                    } else {
                        out.push_str(before);
                        rest = tail;
                    }
                }
            }
            continue;
        }
        // Not followed by an id: keep the prefix as text.
        let keep = if from.starts_with(MEDIA_PREFIX) {
            MEDIA_PREFIX.len()
        } else {
            INTERNAL_LINK_PREFIX.len()
        };
        out.push_str(&rest[..start + keep]);
        rest = &rest[start + keep..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11";
    const B: &str = "5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f";

    #[test]
    fn names_are_valid_everywhere() {
        assert_eq!(safe_name("Kapitel 1: Anfang", "x"), "Kapitel 1_ Anfang");
        assert_eq!(safe_name("a/b\\c|d?*\"<>", "x"), "a_b_c_d_____");
        assert_eq!(safe_name("  Notiz.  ", "x"), "Notiz");
        assert_eq!(safe_name("...", "Unbenannt"), "Unbenannt");
        assert_eq!(safe_name("CON", "x"), "_CON");
        assert_eq!(safe_name("nul.txt", "x"), "_nul.txt");
        assert_eq!(safe_name("Zeile\nzwei\tdrei", "x"), "Zeile_zwei_drei");
        assert_eq!(
            safe_name(&"ä".repeat(300), "x").chars().count(),
            MAX_NAME_CHARS
        );
        assert_eq!(safe_name("Straße & Häuser", "x"), "Straße & Häuser");
    }

    #[test]
    fn repeated_names_are_numbered_without_regard_to_case() {
        let mut names = UniqueNames::default();
        assert_eq!(names.take("Notiz", "md"), "Notiz.md");
        assert_eq!(names.take("notiz", "md"), "notiz (2).md");
        assert_eq!(names.take("Notiz", "md"), "Notiz (3).md");
        assert_eq!(names.take("Notiz", ""), "Notiz");
    }

    #[test]
    fn paths_are_relative_and_encoded() {
        assert_eq!(relative_path("", "Medien/a.png"), "Medien/a.png");
        assert_eq!(
            relative_path("Reise/Tag 1", "Medien/a.png"),
            "../../Medien/a.png"
        );
        assert_eq!(relative_path("Reise", "Reise/Tag 1/b.md"), "Tag 1/b.md");
        assert_eq!(encode_path("Tag 1/Häuser.md"), "Tag%201/H%C3%A4user.md");
    }

    #[test]
    fn links_point_to_the_exported_files() {
        let mut targets = LinkTargets::default();
        targets.media.insert(
            Uuid::try_parse(A).unwrap(),
            "Medien/Skizze 1.png".to_owned(),
        );
        targets
            .documents
            .insert(Uuid::try_parse(B).unwrap(), "../Ziel.md".to_owned());
        let markdown = format!(
            "![Bild](pwmedia://{A}) und [S. 2](pwmedia://{A}#page=2) <img src=\"pwmedia://{A}\" width=\"50%\">\nSiehe [Ziel](pwrite://{B}) und pwrite://kein-id"
        );
        assert_eq!(
            rewrite_links(&markdown, &targets),
            "![Bild](Medien/Skizze%201.png) und [S. 2](Medien/Skizze%201.png#page=2) <img src=\"Medien/Skizze%201.png\" width=\"50%\">\nSiehe [Ziel](../Ziel.md) und pwrite://kein-id"
        );
    }

    #[test]
    fn links_to_documents_not_exported_become_text() {
        let markdown = format!("Vor [dem Ziel](pwrite://{B}) nach, auch <pwrite://{B}>.");
        assert_eq!(
            rewrite_links(&markdown, &LinkTargets::default()),
            "Vor dem Ziel nach, auch ."
        );
    }
}
