/**
 * The minimal schema of plain text and the Markdown source view (section
 * 12: "Reintext und Quelltextansicht nutzen denselben Modus: Tiptap mit
 * Minimal-Schema, Leerzeichen und Tabulatoren bleiben exakt erhalten").
 *
 * One `line` per line of the text, only text in it, no marks, no input
 * rules: what is typed stays exactly as typed. A line per block keeps
 * typing fast in long texts, because only the changed line is drawn again.
 */
import { Node, type AnyExtension, type JSONContent } from '@tiptap/core';
import Document from '@tiptap/extension-document';
import Text from '@tiptap/extension-text';
import { UndoRedo } from '@tiptap/extensions';
import type { Node as ProseMirrorNode, Slice } from '@tiptap/pm/model';

export const Line = Node.create({
  name: 'line',
  group: 'block',
  content: 'text*',
  marks: '',
  // Spaces and tabs as typed; Enter still starts a new line.
  whitespace: 'pre',
  defining: true,

  parseHTML() {
    return [
      { tag: 'div[data-pw-line]', preserveWhitespace: 'full' },
      { tag: 'p', preserveWhitespace: 'full' },
    ];
  },

  renderHTML() {
    return ['div', { 'data-pw-line': '', class: 'pw-line' }, 0];
  },
});

const PlainDocument = Document.extend({ content: 'line+' });

export function plainExtensions(): AnyExtension[] {
  return [PlainDocument, Line, Text, UndoRedo];
}

/** The text as lines of the minimal schema. */
export function plainDocument(text: string): JSONContent {
  return {
    type: 'doc',
    content: text
      .split('\n')
      .map((line) =>
        line === '' ? { type: 'line' } : { type: 'line', content: [{ type: 'text', text: line }] },
      ),
  };
}

/** The lines as text, joined with line feeds. */
export function plainText(document: ProseMirrorNode): string {
  const lines: string[] = [];
  document.forEach((line) => {
    lines.push(line.textContent);
  });
  return lines.join('\n');
}

/** Copied lines as text: one line feed between lines, not a blank line. */
export function plainClipboardText(slice: Slice): string {
  return slice.content.textBetween(0, slice.content.size, '\n');
}
