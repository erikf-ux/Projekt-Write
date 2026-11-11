//! The searchable text of a document (ADR 0020, decision 4).
//!
//! * TXT: the text as it is.
//! * Markdown: the source, without the ids of internal links and media
//!   (`pwrite://<id>`, `pwmedia://<id>#page=<n>`): they are no words of the
//!   author and would match parts of words.
//! * `.pwrite`: the text of every text node, the alternative text and title
//!   of images; one line per block.
//!
//! Control characters (except line break and tab) and the two hit markers of
//! the snippets become spaces.

use serde_json::Value;

use crate::database::search::{HIT_END, HIT_START};
use crate::services::documents::{DocumentFormat, INTERNAL_LINK_PREFIX, MEDIA_PREFIX};
use crate::services::pwrite;

/// The text the index holds for a content.
pub fn searchable_text(format: DocumentFormat, content: &[u8]) -> String {
    let text = match format {
        DocumentFormat::Txt => String::from_utf8_lossy(content).into_owned(),
        DocumentFormat::Md => without_ids(&String::from_utf8_lossy(content)),
        DocumentFormat::Pwrite => match pwrite::parse(content) {
            Ok(pwrite::Parsed::Current(file)) => {
                let mut text = String::new();
                collect_text(&file.content, &mut text);
                text
            }
            Ok(pwrite::Parsed::Newer { content, .. }) => {
                let mut text = String::new();
                collect_text(&content, &mut text);
                text
            }
            Err(_) => String::new(),
        },
    };
    clean(&text)
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch == HIT_START || ch == HIT_END || (ch.is_control() && ch != '\n' && ch != '\t') {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

/// Removes `pwrite://<id>` and `pwmedia://<id>` (with `#page=<n>`).
fn without_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let next = [INTERNAL_LINK_PREFIX, MEDIA_PREFIX]
            .iter()
            .filter_map(|prefix| rest.find(prefix).map(|index| (index, *prefix)))
            .min();
        let Some((index, prefix)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..index]);
        let after = &rest[index + prefix.len()..];
        let id_length = after
            .chars()
            .take(36)
            .take_while(|ch| ch.is_ascii_hexdigit() || *ch == '-')
            .count();
        if id_length == 36 {
            let mut tail = &after[36..];
            if let Some(page) = tail.strip_prefix("#page=") {
                let digits = page.chars().take_while(char::is_ascii_digit).count();
                tail = &page[digits..];
            }
            rest = tail;
        } else {
            // Not an id: keep the text.
            out.push_str(prefix);
            rest = after;
        }
    }
}

fn collect_text(node: &Value, out: &mut String) {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "text" => {
            if let Some(text) = node.get("text").and_then(Value::as_str) {
                out.push_str(text);
            }
            return;
        }
        "hardBreak" => {
            out.push('\n');
            return;
        }
        "image" => {
            for attribute in ["alt", "title"] {
                if let Some(text) = node
                    .get("attrs")
                    .and_then(|attrs| attrs.get(attribute))
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                {
                    separate(out, ' ');
                    out.push_str(text);
                }
            }
            separate(out, ' ');
            return;
        }
        _ => {}
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            collect_text(child, out);
        }
        // Every node with children here is a block (inline content is text,
        // breaks and images).
        if kind != "doc" {
            separate(out, '\n');
        }
    }
}

fn separate(out: &mut String, with: char) {
    if with == '\n' && out.ends_with(' ') {
        // A block ends after an image: the line break replaces the space.
        out.pop();
    }
    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
        out.push(with);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11";

    #[test]
    fn markdown_loses_the_ids_of_links_and_media() {
        let markdown = format!(
            "Siehe [Plan](pwrite://{ID}) und ![Skizze](pwmedia://{ID}) oder [S. 2](pwmedia://{ID}#page=2), pwrite://kein-id"
        );
        assert_eq!(
            searchable_text(DocumentFormat::Md, markdown.as_bytes()),
            "Siehe [Plan]() und ![Skizze]() oder [S. 2](), pwrite://kein-id"
        );
    }

    #[test]
    fn pwrite_text_has_one_line_per_block() {
        let content = serde_json::json!({
            "type": "doc",
            "content": [
                {"type": "heading", "attrs": {"level": 1}, "content": [{"type": "text", "text": "Krankenhaus"}]},
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "Herr "},
                    {"type": "text", "marks": [{"type": "bold"}], "text": "Müller"},
                    {"type": "hardBreak"},
                    {"type": "text", "text": "Straße"},
                    {"type": "image", "attrs": {"src": format!("pwmedia://{ID}"), "alt": "Häuser", "title": "Am Fluss"}}
                ]},
                {"type": "bulletList", "content": [
                    {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "C++"}]}]}
                ]}
            ]
        });
        let file = pwrite::new_file(ID, "Titel", "2026-11-11T10:00:00.000Z", "de", content);
        let text = searchable_text(DocumentFormat::Pwrite, &pwrite::to_bytes(&file));
        assert_eq!(
            text,
            "Krankenhaus\nHerr Müller\nStraße Häuser Am Fluss\nC++\n"
        );
    }

    #[test]
    fn control_characters_and_hit_markers_become_spaces() {
        let text = format!("a\u{0}b{HIT_START}c{HIT_END}d\te\nf");
        assert_eq!(
            searchable_text(DocumentFormat::Txt, text.as_bytes()),
            "a b c d\te\nf"
        );
    }

    #[test]
    fn a_damaged_pwrite_file_has_no_text() {
        assert_eq!(searchable_text(DocumentFormat::Pwrite, b"{not json"), "");
    }
}
