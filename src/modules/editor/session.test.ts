import { afterEach, describe, expect, it, vi } from 'vitest';
import { closeAllSessions, openSession, type SessionOptions } from './session';

function session(format: 'md' | 'txt', text: string, extra: Partial<SessionOptions> = {}) {
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

afterEach(() => {
  closeAllSessions();
});

describe('editor session', () => {
  it('opens Markdown in rich text and saves it unchanged', () => {
    const text = '# Kapitel\n\nEin *kursiver* und ein **fetter** Satz mit [Link](https://example.org).\n';
    const markdown = session('md', text);
    expect(markdown.view).toBe('rich');
    expect(markdown.openIssues).toEqual([]);
    expect(markdown.text()).toBe(text);
    expect(markdown.saveRequest()).toEqual({ kind: 'text', text });
    expect(markdown.root.querySelector('[data-view="rich"] h1')?.textContent).toBe('Kapitel');
  });

  it('opens Markdown it cannot hold in the source view and names the reason', () => {
    const text = '**`a`** ~~b\\😀 c~~\n';
    const markdown = session('md', text);
    expect(markdown.view).toBe('source');
    expect(markdown.openIssues).toEqual([{ kind: 'changed', block: 0, type: 'paragraph' }]);
    expect(markdown.text()).toBe(text);
  });

  it('keeps plain text exactly, spaces and tabs included', () => {
    const text = '  eingerückt\n\tTab\n\nzwei Leerzeilen davor  \n';
    const plain = session('txt', text);
    expect(plain.view).toBe('source');
    expect(plain.text()).toBe(text);
    plain.editor.commands.insertContentAt(1, 'X');
    expect(plain.text()).toBe(`X${text}`);
    // Enter starts a new line; the text gets a line feed.
    plain.editor.commands.setTextSelection(2);
    plain.editor.commands.keyboardShortcut('Enter');
    expect(plain.text()).toBe(`X\n${text}`);
    expect(plain.editor.state.doc.childCount).toBe(6);
  });

  it('switches between rich text and source without changing the text', () => {
    const changes = vi.fn();
    const text = '- [ ] Aufgabe\n- [x] Erledigt\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n';
    const markdown = session('md', text, { onChange: changes });
    const rich = markdown.text();
    expect(markdown.toggleView()).toEqual({ ok: true, issues: [] });
    expect(markdown.view).toBe('source');
    expect(markdown.text()).toBe(rich);
    expect(markdown.toggleView()).toEqual({ ok: true, issues: [] });
    expect(markdown.view).toBe('rich');
    expect(markdown.text()).toBe(rich);
    // Switching is not an edit.
    expect(changes).not.toHaveBeenCalled();
  });

  it('stays in the source view when the text cannot be held as rich text', () => {
    const markdown = session('md', 'Text\n');
    markdown.toggleView();
    markdown.editor.commands.setContent({
      type: 'doc',
      content: [{ type: 'line', content: [{ type: 'text', text: '**`a`** ~~b\\😀 c~~' }] }],
    });
    const result = markdown.toggleView();
    expect(result.ok).toBe(false);
    expect(result.issues).toEqual([{ kind: 'changed', block: 0, type: 'paragraph' }]);
    expect(markdown.view).toBe('source');
  });

  it('blocks saving what Markdown would lose, until the user accepts it', () => {
    const markdown = session('md', 'Text\n');
    const cell = (colspan: number) => ({
      type: 'tableHeader',
      attrs: { colspan, rowspan: 1, colwidth: null },
      content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Kopf' }] }],
    });
    markdown.editor.commands.setContent({
      type: 'doc',
      content: [{ type: 'table', content: [{ type: 'tableRow', content: [cell(2)] }] }],
    });
    const request = markdown.saveRequest();
    expect(request.kind).toBe('blocked');
    const losses = request.kind === 'blocked' ? request.losses : [];
    expect(losses.map((loss) => loss.kind)).toEqual(['tableMergedCells']);
    markdown.acceptLosses(losses);
    expect(markdown.saveRequest().kind).toBe('text');
  });

  it('does not warn about blocks the file had and the probe checked', () => {
    // Raw HTML at the start of a paragraph reads back the same here.
    const text = '<span>Hinweis</span> im Text.\n';
    const markdown = session('md', text);
    expect(markdown.view).toBe('rich');
    expect(markdown.saveRequest()).toEqual({ kind: 'text', text });
  });

  it('reports edits, follows the title and the read-only state', () => {
    const changes = vi.fn();
    const markdown = session('md', 'Text\n', { onChange: changes });
    markdown.editor.commands.insertContentAt(1, 'Neu ');
    expect(changes).toHaveBeenCalledTimes(1);
    markdown.setLabel('Schreibfläche: Neu');
    expect(markdown.editor.view.dom.getAttribute('aria-label')).toBe('Schreibfläche: Neu');
    markdown.setEditable(false);
    expect(markdown.editor.isEditable).toBe(false);
  });
});
