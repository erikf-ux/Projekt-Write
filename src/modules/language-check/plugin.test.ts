import { Editor, type JSONContent } from '@tiptap/core';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { markdownExtensions } from '@/modules/editor/schema/extensions';
import { useUi } from '@/modules/settings/uiStore';
import type { UiState } from '@/types/generated/UiState';
import { MASK, blockText, wordBefore } from './extract';
import { DEFAULT_CHECK_SETTINGS, useLanguage } from './languageStore';
import { LanguageCheck, clearCheckCache, findingAt, replaceFinding } from './plugin';

const DOC = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

interface Calls {
  checked: { text: string; language?: string }[];
  autocorrect: { word: string; span: string }[];
  rejected: { word: string; correction: string }[];
}

let calls: Calls;
let corrections: Record<string, string>;

/** A tiny stand-in for the Rust check: flags `Fesnter` and double spaces. */
function check(text: string) {
  const findings = [];
  for (const match of text.matchAll(/Fesnter|Worterbuch/g)) {
    findings.push({
      start: match.index,
      end: match.index + match[0].length,
      kind: 'spelling',
      rule: 'spelling',
      message: null,
      replacements: [],
    });
  }
  for (const match of text.matchAll(/ {2,}/g)) {
    findings.push({
      start: match.index,
      end: match.index + match[0].length,
      kind: 'punctuation',
      rule: 'doubleSpace',
      message: null,
      replacements: [' '],
    });
  }
  return { language: 'de-DE', dictionary: 'de-DE', findings };
}

function settings(autocorrect: boolean, enabled = true) {
  useUi.setState({
    state: {
      settings: { languageCheck: { ...DEFAULT_CHECK_SETTINGS, autocorrect, enabled } },
      locale: 'de',
    } as unknown as UiState,
  });
}

function editor(content: string | JSONContent, documentId: string | null = DOC) {
  const element = document.createElement('div');
  document.body.append(element);
  return new Editor({
    element,
    extensions: [...markdownExtensions(), LanguageCheck.configure({ documentId, enabled: true })],
    content,
  });
}

/** Types like ProseMirror does: `handleTextInput` first, then the insertion. */
function type(target: Editor, text: string) {
  const { view } = target;
  for (const character of text) {
    const { from, to } = view.state.selection;
    const handled = view.someProp('handleTextInput', (f) =>
      f(view, from, to, character, () => view.state.tr.insertText(character, from, to)),
    );
    if (!handled) {
      view.dispatch(view.state.tr.insertText(character, from, to));
    }
  }
}

function underlined(target: Editor): string[] {
  return [...target.view.dom.querySelectorAll('.pw-check')].map((element) => element.textContent);
}

beforeEach(() => {
  vi.useFakeTimers();
  clearCheckCache();
  calls = { checked: [], autocorrect: [], rejected: [] };
  corrections = { Fesnter: 'Fenster' };
  useLanguage.setState({ profiles: {}, ignored: new Set(), epoch: 0 });
  settings(false);
  mockIPC((command, args) => {
    const request = (args as { request: Record<string, unknown> }).request;
    switch (command) {
      case 'language_profile':
        return { languages: [], autocorrect: true };
      case 'language_check': {
        const texts = request['texts'] as { text: string; language?: string }[];
        calls.checked.push(...texts);
        return texts.map((t) => check(t.text));
      }
      case 'language_autocorrect': {
        const word = String(request['word']);
        calls.autocorrect.push({ word, span: String(request['span']) });
        const correction = corrections[word] ?? null;
        return { correction, blockedBy: correction ? null : 3 };
      }
      case 'language_reject':
        calls.rejected.push(request as { word: string; correction: string });
        return null;
      default:
        return null;
    }
  });
});

afterEach(() => {
  clearMocks();
  vi.useRealTimers();
  document.body.innerHTML = '';
});

describe('check text of a paragraph', () => {
  it('masks inline code and links and keeps the offsets', () => {
    const target = editor('<p>Nimm <code>npm ci</code> und <a href="https://x.org">den Link</a>.</p>');
    const block = blockText(target.state.doc.child(0));
    expect(block?.text).toBe(`Nimm ${MASK.repeat(6)} und ${MASK.repeat(8)}.`);
    expect(block?.parts).toHaveLength(1);
    target.destroy();
  });

  it('makes a part marked with a language a text of its own', () => {
    const target = editor('<p>Er sagte <span lang="en-GB">good morning</span> und ging.</p>');
    expect(blockText(target.state.doc.child(0))?.parts).toEqual([
      { offset: 0, text: 'Er sagte ', language: null },
      { offset: 9, text: 'good morning', language: 'en-GB' },
      { offset: 21, text: ' und ging.', language: null },
    ]);
    target.destroy();
  });

  it('skips code blocks and finds the word before a position', () => {
    const target = editor('<pre><code>Fesnter</code></pre>');
    expect(blockText(target.state.doc.child(0))).toBeNull();
    expect(wordBefore('Das E-Mail-Fesnter', 18)).toEqual({ start: 4, word: 'E-Mail-Fesnter' });
    expect(wordBefore('geht’s ', 6)).toEqual({ start: 0, word: 'geht’s' });
    expect(wordBefore('Text ', 5)).toBeNull();
    target.destroy();
  });
});

describe('language check in the editor', () => {
  it('underlines findings after a pause and keeps code out of the check', async () => {
    const target = editor({
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: 'Das Fesnter  ist offen.' }] },
        { type: 'codeBlock', content: [{ type: 'text', text: 'Fesnter' }] },
      ],
    });
    await vi.advanceTimersByTimeAsync(500);
    expect(calls.checked).toEqual([{ text: 'Das Fesnter  ist offen.' }]);
    expect(underlined(target)).toEqual(['Fesnter', '  ']);
    expect(target.view.dom.querySelector('.pw-check-spelling')?.getAttribute('data-check-rule')).toBe(
      'spelling',
    );
    // The finding under a position, and its replacement as one undo step.
    const space = findingAt(target.state, 13);
    expect(space?.finding.rule).toBe('doubleSpace');
    if (space) {
      replaceFinding(target.view, space, ' ');
    }
    expect(target.state.doc.textContent).toBe('Das Fesnter ist offen.Fesnter');
    target.destroy();
  });

  it('takes the underlines of an edited paragraph away until it is checked again', async () => {
    const target = editor('<p>Das Fesnter ist offen.</p><p>Fesnter zwei.</p>');
    await vi.advanceTimersByTimeAsync(500);
    expect(underlined(target)).toEqual(['Fesnter', 'Fesnter']);
    const before = calls.checked.length;
    target.commands.setTextSelection(2);
    type(target, 'x');
    expect(underlined(target)).toEqual(['Fesnter']);
    await vi.advanceTimersByTimeAsync(500);
    expect(underlined(target)).toEqual(['Fesnter', 'Fesnter']);
    // Only the edited paragraph is checked again; the other comes from the cache.
    expect(calls.checked.slice(before).map((c) => c.text)).toEqual(['Dxas Fesnter ist offen.']);
    target.destroy();
  });

  it('checks nothing when the language check is off', async () => {
    settings(false, false);
    const target = editor('<p>Das Fesnter.</p>');
    await vi.advanceTimersByTimeAsync(500);
    expect(calls.checked).toEqual([]);
    expect(underlined(target)).toEqual([]);
    target.destroy();
  });
});

describe('autocorrect', () => {
  it('corrects a finished word as its own undo step; Ctrl+Z restores and rejects it', async () => {
    settings(true);
    const target = editor('<p></p>');
    await vi.advanceTimersByTimeAsync(10);
    target.commands.setTextSelection(1);
    type(target, 'Das Fesnter ');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.autocorrect.map((c) => c.word)).toEqual(['Fesnter']);
    expect(target.state.doc.textContent).toBe('Das Fenster ');
    expect(target.view.dom.querySelector('.pw-check-corrected')?.textContent).toBe('Fenster');
    await vi.advanceTimersByTimeAsync(2000);
    expect(target.view.dom.querySelector('.pw-check-corrected')).toBeNull();
    // Typing on is a step of its own.
    vi.setSystemTime(Date.now() + 1000);
    type(target, 'ist');
    target.commands.undo();
    expect(target.state.doc.textContent).toBe('Das Fenster ');
    target.commands.undo();
    expect(target.state.doc.textContent).toBe('Das Fesnter ');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.rejected).toEqual([{ word: 'Fesnter', correction: 'Fenster' }]);
    target.destroy();
  });

  it('drops a late answer when the word changed in between', async () => {
    settings(true);
    let answer: (value: unknown) => void = () => undefined;
    mockIPC((command, args) => {
      const request = (args as { request: Record<string, unknown> }).request;
      if (command === 'language_autocorrect') {
        calls.autocorrect.push({ word: String(request['word']), span: String(request['span']) });
        return new Promise((resolve) => {
          answer = resolve;
        });
      }
      return command === 'language_check' ? [] : null;
    });
    const target = editor('<p></p>');
    target.commands.setTextSelection(1);
    type(target, 'Fesnter ');
    await vi.advanceTimersByTimeAsync(1);
    target.commands.setTextSelection(3);
    type(target, 'x');
    answer({ correction: 'Fenster', blockedBy: null });
    await vi.advanceTimersByTimeAsync(10);
    expect(target.state.doc.textContent).toBe('Fexsnter ');
    target.destroy();
  });

  it('asks nothing while the autocorrect is off, for short words and in quotations', async () => {
    const target = editor('<p></p><blockquote><p></p></blockquote>');
    target.commands.setTextSelection(1);
    type(target, 'Fesnter ');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.autocorrect).toEqual([]);
    settings(true);
    type(target, 'Hsu ');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.autocorrect).toEqual([]);
    target.commands.setTextSelection(target.state.doc.content.size - 2);
    type(target, 'Fesnter.');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.autocorrect).toEqual([{ word: 'Fesnter', span: 'quote' }]);
    target.destroy();
  });

  it('waits for the end of a composition', async () => {
    settings(true);
    const target = editor('<p></p>');
    target.commands.setTextSelection(1);
    const view = target.view;
    Object.defineProperty(view, 'composing', { value: true, configurable: true });
    type(target, 'Fesnter ');
    await vi.advanceTimersByTimeAsync(10);
    expect(calls.autocorrect).toEqual([]);
    target.destroy();
  });
});
