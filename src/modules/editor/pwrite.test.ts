/**
 * `.pwrite` in the editor (section 10, `docs/pwrite-format.md`): the content
 * round-trips through the editor, also what Markdown cannot hold.
 */
import type { JSONContent } from '@tiptap/core';
import { afterEach, describe, expect, it } from 'vitest';
import { markdownSchema } from './markdown/probe';
import { closeAllSessions, openSession, pwriteContent } from './session';

function session(text: string) {
  const created = openSession({
    id: `doc-pwrite-${String(Math.random())}`,
    format: 'pwrite',
    text,
    label: 'Schreibfläche: Test',
    editable: true,
    onChange: () => undefined,
    onBlur: () => undefined,
  });
  document.body.append(created.root);
  return created;
}

const paragraph = (text: string, marks?: JSONContent['marks']): JSONContent => ({
  type: 'paragraph',
  content: [{ type: 'text', text, ...(marks ? { marks } : {}) }],
});

/** Everything the editor holds, including what Markdown would lose. */
const CONTENT: JSONContent = {
  type: 'doc',
  content: [
    { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Kapitel' }] },
    paragraph('fett', [{ type: 'bold' }, { type: 'underline' }]),
    paragraph('intern', [{ type: 'link', attrs: { href: 'pwrite://0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f' } }]),
    {
      type: 'orderedList',
      attrs: { start: 3, type: 'a' },
      content: [{ type: 'listItem', content: [paragraph('Buchstaben')] }],
    },
    {
      type: 'table',
      content: [
        {
          type: 'tableRow',
          content: [
            { type: 'tableHeader', attrs: { colspan: 2, align: 'center' }, content: [paragraph('Kopf')] },
          ],
        },
        {
          type: 'tableRow',
          content: [
            { type: 'tableCell', attrs: { colwidth: [120] }, content: [paragraph('a'), paragraph('b')] },
            { type: 'tableCell', content: [paragraph('c')] },
          ],
        },
      ],
    },
    { type: 'rawBlock', attrs: { raw: '<div>roh</div>', kind: 'html' } },
    { type: 'codeBlock', attrs: { language: 'rust' }, content: [{ type: 'text', text: 'fn main() {}' }] },
  ],
};

afterEach(() => {
  closeAllSessions();
});

describe('.pwrite in the editor', () => {
  it('round-trips the content, also what Markdown cannot hold', () => {
    const editor = session(JSON.stringify(CONTENT));
    expect(editor.view).toBe('rich');
    expect(editor.openIssues).toEqual([]);
    const saved = JSON.parse(editor.text()) as JSONContent;
    expect(saved).toEqual(markdownSchema().nodeFromJSON(CONTENT).toJSON());
    // Reading the saved content gives the same document again.
    expect(JSON.parse(session(editor.text()).text())).toEqual(saved);
    // Nothing to warn about: the format holds everything.
    expect(editor.losses()).toEqual([]);
    expect(editor.saveRequest()).toEqual({ kind: 'text', text: editor.text() });
  });

  it('has no source view', () => {
    const editor = session(JSON.stringify(CONTENT));
    expect(editor.isMarkdown).toBe(false);
    expect(editor.toggleView().ok).toBe(false);
    expect(editor.view).toBe('rich');
  });

  it('names content that does not fit the schema', () => {
    expect(pwriteContent('{"type":"doc","content":[{"type":"wonder"}]}')).toBeNull();
    expect(pwriteContent('kein JSON')).toBeNull();
    const editor = session('{"type":"doc","content":[{"type":"wonder"}]}');
    expect(editor.openIssues).toEqual([{ kind: 'invalid' }]);
  });
});
