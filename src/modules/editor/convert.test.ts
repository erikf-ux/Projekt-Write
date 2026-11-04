import type { JSONContent } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import { convertContent, needsConfirmation, paragraphsFromText, plainTextOf } from './convert';
import { parseMarkdown } from './markdown/adapter';

const RICH: JSONContent = {
  type: 'doc',
  content: [
    { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: 'Kapitel' }] },
    {
      type: 'paragraph',
      content: [
        { type: 'text', text: 'Ein ' },
        { type: 'text', text: 'fetter', marks: [{ type: 'bold' }] },
        { type: 'text', text: ' Satz.' },
        { type: 'hardBreak' },
        { type: 'text', text: 'Zweite Zeile.' },
      ],
    },
  ],
};

const MERGED_TABLE: JSONContent = {
  type: 'doc',
  content: [
    {
      type: 'table',
      content: [
        {
          type: 'tableRow',
          content: [
            {
              type: 'tableHeader',
              attrs: { colspan: 2, rowspan: 1, colwidth: null },
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Kopf' }] }],
            },
          ],
        },
        {
          type: 'tableRow',
          content: [
            {
              type: 'tableCell',
              attrs: { colspan: 1, rowspan: 1, colwidth: null },
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'a' }] }],
            },
            {
              type: 'tableCell',
              attrs: { colspan: 1, rowspan: 1, colwidth: null },
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'b' }] }],
            },
          ],
        },
      ],
    },
  ],
};

describe('convertContent', () => {
  it('makes paragraphs of plain text: blank lines separate them, line feeds stay', () => {
    expect(paragraphsFromText('Eins\nzwei\n\n\nDrei')).toEqual({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [{ type: 'text', text: 'Eins' }, { type: 'hardBreak' }, { type: 'text', text: 'zwei' }],
        },
        { type: 'paragraph', content: [{ type: 'text', text: 'Drei' }] },
      ],
    });
    expect(paragraphsFromText('')).toEqual({ type: 'doc', content: [{ type: 'paragraph' }] });
  });

  it('turns text into Markdown that reads back as the same text', () => {
    const text = 'Preis: 5 * 3 = 15\n# keine Überschrift\n\n_kein_ Kursiv und [kein](Link)';
    const conversion = convertContent('txt', 'md', { kind: 'text', text });
    expect(needsConfirmation(conversion)).toBe(false);
    expect(plainTextOf(parseMarkdown(conversion.text).body)).toBe(text);
  });

  it('turns text into rich text without asking', () => {
    const conversion = convertContent('txt', 'pwrite', { kind: 'text', text: 'Eins\n\nZwei' });
    expect(needsConfirmation(conversion)).toBe(false);
    expect(JSON.parse(conversion.text)).toEqual(paragraphsFromText('Eins\n\nZwei'));
  });

  it('keeps Markdown text as it is when it becomes plain text', () => {
    const text = '---\ntitel: A\n---\n# Kapitel\n\n**fett**\n';
    const conversion = convertContent('md', 'txt', { kind: 'text', text });
    expect(conversion.text).toBe(text);
    expect(needsConfirmation(conversion)).toBe(false);
  });

  it('takes the rich text of a Markdown tab into .pwrite and names a front matter', () => {
    const conversion = convertContent('md', 'pwrite', { kind: 'rich', content: RICH, frontMatter: '' });
    expect(JSON.parse(conversion.text)).toEqual(RICH);
    expect(needsConfirmation(conversion)).toBe(false);

    const source = convertContent('md', 'pwrite', { kind: 'text', text: '---\ntitel: A\n---\n# Kapitel\n' });
    expect(source.frontMatterLost).toBe(true);
    expect(needsConfirmation(source)).toBe(true);
    expect((JSON.parse(source.text) as JSONContent).content?.[0]?.type).toBe('heading');
  });

  it('names what Markdown cannot hold when rich text becomes Markdown', () => {
    const clean = convertContent('pwrite', 'md', { kind: 'rich', content: RICH, frontMatter: '' });
    expect(clean.losses).toEqual([]);
    expect(clean.text).toContain('## Kapitel');
    expect(clean.text).toContain('**fetter**');

    const table = convertContent('pwrite', 'md', { kind: 'rich', content: MERGED_TABLE, frontMatter: '' });
    expect(table.losses.map((loss) => loss.kind)).toContain('tableMergedCells');
    expect(needsConfirmation(table)).toBe(true);
  });

  it('keeps the text and loses the formatting when rich text becomes plain text', () => {
    const conversion = convertContent('pwrite', 'txt', { kind: 'rich', content: RICH, frontMatter: '' });
    expect(conversion.text).toBe('Kapitel\n\nEin fetter Satz.\nZweite Zeile.');
    expect(conversion.formattingLost).toBe(true);
    expect(needsConfirmation(conversion)).toBe(true);
  });
});
