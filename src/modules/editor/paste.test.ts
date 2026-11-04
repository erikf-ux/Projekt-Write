import { afterEach, describe, expect, it } from 'vitest';
import { looksLikeMarkdown, notePlainPasteChord } from './paste';
import { closeAllSessions, openSession } from './session';

function richSession(text = 'Start\n') {
  const session = openSession({
    id: `paste-${String(Math.random())}`,
    format: 'md',
    text,
    label: 'Test',
    editable: true,
    onChange: () => undefined,
    onBlur: () => undefined,
  });
  document.body.append(session.root);
  return session;
}

/** A paste event as ProseMirror's handlePaste sees it. */
function paste(session: ReturnType<typeof richSession>, data: Record<string, string>): void {
  const event = {
    clipboardData: {
      types: Object.keys(data),
      getData: (type: string) => data[type] ?? '',
    },
  } as unknown as ClipboardEvent;
  const view = session.editor.view;
  // Put the cursor at the end of the document first.
  session.editor.commands.focus('end');
  const handled = view.someProp('handlePaste', (handler) =>
    handler(view, event, view.state.selection.content()),
  );
  if (!handled && data['text/html'] !== undefined) {
    view.pasteHTML(data['text/html']);
  } else if (!handled && data['text/plain'] !== undefined) {
    view.pasteText(data['text/plain']);
  }
}

afterEach(() => {
  closeAllSessions();
});

describe('pasting', () => {
  it.each([
    ['# Titel', true],
    ['- Punkt\n- Punkt', true],
    ['1. Erstens', true],
    ['> Zitat', true],
    ['```\ncode\n```', true],
    ['| a | b |\n| --- | --- |', true],
    ['Ein **fetter** Satz', true],
    ['Siehe [Link](https://example.org)', true],
    ['Mit `Code` darin', true],
    ['Ein ganz normaler Satz.', false],
    ['Preis: 3 * 4 = 12, #hashtag, 2024.', false],
    ['Zeile eins\nZeile zwei', false],
  ])('recognises Markdown: %j', (text, expected) => {
    expect(looksLikeMarkdown(text)).toBe(expected);
  });

  it('reads pasted HTML with the schema: no colours, images as notes, safe links only', () => {
    const session = richSession();
    paste(session, {
      'text/html':
        '<p><span style="color:red">rot</span> <b>fett</b> <img src="https://example.org/bild.png" alt="Bild"> <a href="javascript:alert(1)">böse</a> <a href="https://example.org">gut</a></p>',
      'text/plain': 'rot fett böse gut',
    });
    const markdown = session.text();
    expect(markdown).toContain('rot **fett** ![Bild](https://example.org/bild.png)');
    expect(markdown).toContain('[gut](https://example.org)');
    expect(markdown).not.toContain('javascript');
    expect(markdown).not.toContain('color');
    // The image is a note: nothing in the editor would load it.
    expect(session.root.querySelector('img')).toBeNull();
  });

  it('reads pasted Markdown text as Markdown', () => {
    const session = richSession();
    paste(session, { 'text/plain': '## Abschnitt\n\nMit **fett** und [Link](https://example.org).' });
    expect(session.root.querySelector('h2')?.textContent).toBe('Abschnitt');
    expect(session.text()).toContain('## Abschnitt\n\nMit **fett** und [Link](https://example.org).');
  });

  it('pastes plain text after Ctrl+Shift+V, even with HTML on the clipboard', () => {
    const session = richSession();
    notePlainPasteChord(new KeyboardEvent('keydown', { key: 'V', ctrlKey: true, shiftKey: true }));
    paste(session, { 'text/html': '<h1>Groß</h1>', 'text/plain': '# Groß' });
    expect(session.root.querySelector('h1')).toBeNull();
    // One line of text joins the paragraph at the cursor, as typed text would.
    expect(session.text()).toBe('Start# Groß\n');
  });
});
