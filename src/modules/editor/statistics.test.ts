import { Editor } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { afterEach, describe, expect, it } from 'vitest';
import { parseMarkdown } from './markdown/adapter';
import { plainDocument, plainExtensions } from './plain/schema';
import { markdownExtensions } from './schema/extensions';
import {
  countText,
  plainStatistics,
  readingMinutes,
  richStatistics,
  selectionStatistics,
  type TextStatistics,
} from './statistics';

const editors: Editor[] = [];

function richEditor(markdown: string): Editor {
  const editor = new Editor({ extensions: markdownExtensions(), content: parseMarkdown(markdown).body });
  editors.push(editor);
  return editor;
}

function plainEditor(text: string): Editor {
  const editor = new Editor({ extensions: plainExtensions(), content: plainDocument(text) });
  editors.push(editor);
  return editor;
}

afterEach(() => {
  for (const editor of editors.splice(0)) {
    editor.destroy();
  }
});

describe('countText (German texts, Unicode rules)', () => {
  it.each([
    ['Größe, Maß und „Zitat“ – Gedankenstrich.', 5, 40, 35],
    ['Die Straße heißt Kurfürstendamm.', 4, 32, 29],
    ['Er sagte: „Geht’s heute?“', 4, 25, 22],
    ['Das kostet 2,50 € oder 3.000,00 €.', 5, 34, 28],
    ['Bitte per E-Mail, z. B. an Frau Dr. Müller.', 10, 43, 35],
    ['GROẞBUCHSTABE und Mädchen', 3, 25, 23],
    ['Daumen 👍🏽 und Akzent é', 4, 21, 17],
    ['  Leerzeichen\tund Tab  ', 3, 23, 17],
    ['', 0, 0, 0],
  ])('%j', (text, words, characters, withoutSpaces) => {
    expect(countText(text)).toEqual({ words, characters, charactersWithoutSpaces: withoutSpaces });
  });
});

describe('document statistics', () => {
  const markdown = [
    '# Kapitel eins',
    '',
    'Es war einmal ein *kleiner* Text.  ',
    'Mit hartem Umbruch.',
    '',
    '- Erster Punkt',
    '- Zweiter Punkt',
    '',
    '&nbsp;',
    '',
    '| Spalte | Wert |',
    '| --- | --- |',
    '| Eins | 1 |',
    '',
    '<div>kein Text</div>',
    '',
    'Fußnote[^1].',
    '',
    '[^1]: Die Anmerkung.',
    '',
  ].join('\n');

  it('counts words, characters and paragraphs of rich text', () => {
    const editor = richEditor(markdown);
    const statistics = richStatistics(editor.state.doc, new WeakMap());
    // Heading 2 words; paragraph 9; list 4; table 4 ("1" is a word);
    // "Fußnote" 1 (the reference is markup); footnote text 2.
    expect(statistics.words).toBe(22);
    // Heading, paragraph, two list items, four cells, footnote paragraph,
    // footnote definition; the empty paragraph and the HTML block are none.
    expect(statistics.paragraphs).toBe(10);
    expect(statistics.charactersWithoutSpaces).toBeLessThan(statistics.characters);
  });

  it('keeps the counts of unchanged blocks', () => {
    const editor = richEditor('Eins zwei.\n\nDrei.\n');
    const cache = new WeakMap<ProseMirrorNode, TextStatistics>();
    expect(richStatistics(editor.state.doc, cache).words).toBe(3);
    editor.commands.insertContentAt(editor.state.doc.content.size - 1, ' vier');
    expect(richStatistics(editor.state.doc, cache).words).toBe(4);
    expect(cache.get(editor.state.doc.child(0))).toEqual({
      words: 2,
      characters: 10,
      charactersWithoutSpaces: 9,
      paragraphs: 1,
    });
  });

  it('counts paragraphs of plain text as groups of lines', () => {
    const editor = plainEditor('Zeile eins\nZeile zwei\n\n\nNeuer Absatz\n   \nLetzter');
    expect(plainStatistics(editor.state.doc, new WeakMap())).toEqual({
      words: 7,
      characters: 42,
      charactersWithoutSpaces: 36,
      paragraphs: 3,
    });
  });

  it('counts the selection only', () => {
    const editor = richEditor('Erster Absatz hier.\n\nZweiter Absatz dort.\n');
    // From "Absatz" in the first paragraph to "Zweiter" in the second.
    const from = 1 + 'Erster '.length;
    const to = editor.state.doc.child(0).nodeSize + 1 + 'Zweiter'.length;
    expect(selectionStatistics(editor.state.doc, from, to, false)).toEqual({
      words: 3,
      characters: 19,
      charactersWithoutSpaces: 18,
      paragraphs: 2,
    });
    const plain = plainEditor('a b\n\nc d');
    expect(selectionStatistics(plain.state.doc, 0, plain.state.doc.content.size, true).paragraphs).toBe(2);
  });
});

describe('reading time', () => {
  it('rounds to whole minutes at the chosen speed', () => {
    expect(readingMinutes(0, 200)).toBe(0);
    expect(readingMinutes(10, 200)).toBe(1);
    expect(readingMinutes(450, 200)).toBe(2);
    expect(readingMinutes(50_000, 200)).toBe(250);
    expect(readingMinutes(50_000, 250)).toBe(200);
  });
});
