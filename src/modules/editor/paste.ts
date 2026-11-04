/**
 * Pasting into rich text (section 12: "Eingefügtes HTML durchläuft das
 * Editor-Schema. `Ctrl + Shift + V` fügt reinen Text ein."; "Kürzel und
 * eingefügtes Markdown wirken auch …").
 *
 * * HTML: ProseMirror reads it with the schema; what the schema cannot hold
 *   (colours, fonts, scripts) is left out. Images become notes and are never
 *   loaded; links keep only `http`, `https` and `mailto`.
 * * Text that looks like Markdown is read as Markdown.
 * * `Ctrl+Shift+V`: the text only. Not every webview pastes plain text on
 *   this chord, so the editor remembers it for the paste that follows.
 */
import type { JSONContent } from '@tiptap/core';
import type { EditorView } from '@tiptap/pm/view';
import { parseMarkdown } from './markdown/adapter';

/** Markdown blocks at the start of a line, or inline syntax with its pair. */
const MARKDOWN = [
  /^ {0,3}#{1,6}[ \t]+\S/m,
  /^ {0,3}(?:[-*+]|\d{1,9}[.)])[ \t]+\S/m,
  /^ {0,3}>/m,
  /^ {0,3}(?:```|~~~)/m,
  /^ {0,3}\|.*\|[ \t]*\n {0,3}\|?[ \t]*:?-{3,}/m,
  /\*\*[^*\n]+\*\*|__[^_\n]+__|~~[^~\n]+~~/,
  /!?\[[^\]\n]+\]\([^)\s]+\)/,
  /`[^`\n]+`/,
];

export function looksLikeMarkdown(text: string): boolean {
  return MARKDOWN.some((pattern) => pattern.test(text));
}

/** The blocks of pasted Markdown, ready to insert. */
export function markdownContent(text: string): JSONContent[] {
  return parseMarkdown(text.replace(/\r\n?/g, '\n')).body.content ?? [];
}

/** How long after `Ctrl+Shift+V` a paste counts as plain text. */
const PLAIN_PASTE_WINDOW_MS = 1000;

let plainPasteAt = Number.NEGATIVE_INFINITY;

/** Remembers `Ctrl+Shift+V` (installed once for the window). */
export function notePlainPasteChord(event: KeyboardEvent): void {
  if (
    (event.ctrlKey || event.metaKey) &&
    event.shiftKey &&
    !event.altKey &&
    event.key.toLowerCase() === 'v'
  ) {
    plainPasteAt = performance.now();
  }
}

function takePlainPaste(): boolean {
  const requested = performance.now() - plainPasteAt <= PLAIN_PASTE_WINDOW_MS;
  plainPasteAt = Number.NEGATIVE_INFINITY;
  return requested;
}

/**
 * `handlePaste` of the rich editor. `insertMarkdown` inserts parsed blocks
 * at the selection. Returns true if it handled the paste.
 */
export function handleRichPaste(
  view: EditorView,
  event: Pick<ClipboardEvent, 'clipboardData'>,
  insertMarkdown: (content: JSONContent[]) => void,
): boolean {
  const data = event.clipboardData;
  if (!data) {
    return false;
  }
  const text = data.getData('text/plain').replace(/\r\n?/g, '\n');
  if (takePlainPaste()) {
    if (text === '') {
      return false;
    }
    view.pasteText(text);
    return true;
  }
  if (Array.from(data.types).includes('text/html')) {
    // Through the schema (ProseMirror's own clipboard parser).
    return false;
  }
  if (text !== '' && looksLikeMarkdown(text)) {
    insertMarkdown(markdownContent(text));
    return true;
  }
  return false;
}
