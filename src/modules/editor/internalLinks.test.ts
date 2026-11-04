import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { matchingDocuments } from './InternalLinkPicker';
import { isAllowedLink } from './LinkDialog';
import { internalLinkTarget } from './schema/extensions';
import { closeAllSessions, linkAt, openSession, type SessionOptions } from './session';

const ID = '0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f';

function summary(id: string, title: string): DocumentSummary {
  return {
    id,
    title,
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-04T10:00:00.000Z',
    updatedAt: '2026-11-04T10:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
  };
}

function session(format: 'md' | 'pwrite', text: string, extra: Partial<SessionOptions> = {}) {
  const created = openSession({
    id: `doc-${format}-${String(Math.random())}`,
    format,
    text,
    label: 'Schreibfläche: Test',
    editable: true,
    onChange: () => undefined,
    onBlur: () => undefined,
    ...extra,
  });
  document.body.append(created.root);
  return created;
}

/** Text typed at the cursor, as ProseMirror reports it to `handleTextInput`. */
function typeAtCursor(created: ReturnType<typeof session>, text: string): void {
  const { view } = created.editor;
  for (const character of text) {
    const { from, to } = view.state.selection;
    const handled = view.someProp('handleTextInput', (handle) =>
      handle(view, from, to, character, () => view.state.tr),
    );
    if (handled !== true) {
      view.dispatch(view.state.tr.insertText(character, from, to));
    }
  }
}

afterEach(() => {
  closeAllSessions();
  vi.useRealTimers();
});

describe('internal links', () => {
  it('accepts pwrite://<id> only with a document id', () => {
    expect(internalLinkTarget(`pwrite://${ID}`)).toBe(ID);
    for (const href of ['pwrite://', `pwrite://${ID.toUpperCase()}`, `pwrite://${ID}/x`, `https://${ID}`]) {
      expect(internalLinkTarget(href), href).toBeNull();
    }
    expect(isAllowedLink(`pwrite://${ID}`)).toBe(true);
    expect(isAllowedLink('pwrite://kapitel-2')).toBe(false);
  });

  it('finds documents by every word of the query, without the current one', () => {
    const documents = [summary('a', 'Kapitel Zwei'), summary('b', 'kapitel eins'), summary('c', 'Notizen')];
    expect(matchingDocuments(documents, '', 'c', 'de').map((d) => d.title)).toEqual([
      'kapitel eins',
      'Kapitel Zwei',
    ]);
    expect(matchingDocuments(documents, 'KAP zw', 'x', 'de').map((d) => d.id)).toEqual(['a']);
    expect(matchingDocuments(documents, 'fehlt', 'x', 'de')).toEqual([]);
  });

  it('asks for a document after [[ and writes the link as Markdown', () => {
    vi.useFakeTimers();
    const asked = vi.fn();
    const markdown = session('md', 'Siehe\n', { onInternalLinkRequest: asked });
    markdown.editor.commands.setTextSelection(6);
    typeAtCursor(markdown, ' [');
    vi.runAllTimers();
    expect(asked).not.toHaveBeenCalled();
    typeAtCursor(markdown, '[');
    vi.runAllTimers();
    expect(asked).toHaveBeenCalledTimes(1);

    markdown.insertInternalLink(ID, 'Kapitel 2');
    expect(markdown.text()).toBe(`Siehe [Kapitel 2](pwrite://${ID})\n`);
    expect(linkAt(markdown.editor.state.doc, 9)).toBe(`pwrite://${ID}`);
    // Typing after the link does not extend it.
    markdown.editor.commands.insertContent('.');
    expect(markdown.text()).toBe(`Siehe [Kapitel 2](pwrite://${ID}).\n`);
  });

  it('keeps internal links in .pwrite and in Markdown read from a file', () => {
    const rich = session('pwrite', JSON.stringify({ type: 'doc', content: [{ type: 'paragraph' }] }));
    rich.insertInternalLink(ID, 'Ziel');
    expect(rich.text()).toContain(`"href":"pwrite://${ID}"`);
    const text = `Ein [Ziel](pwrite://${ID}).\n`;
    const markdown = session('md', text);
    expect(markdown.view).toBe('rich');
    expect(markdown.text()).toBe(text);
  });

  it('does not ask inside code', () => {
    vi.useFakeTimers();
    const asked = vi.fn();
    const markdown = session('md', 'Text `code` Ende\n', { onInternalLinkRequest: asked });
    markdown.editor.commands.setTextSelection(8);
    typeAtCursor(markdown, '[[');
    vi.runAllTimers();
    expect(asked).not.toHaveBeenCalled();
  });
});
