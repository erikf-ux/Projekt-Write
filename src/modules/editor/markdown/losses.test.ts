import type { JSONContent } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import { parseMarkdown, serializeMarkdown } from './adapter';
import { findLosses, type LossKind } from './losses';
import { probeMarkdown } from './probe';

const text = (value: string, marks: JSONContent['marks'] = []): JSONContent =>
  marks.length > 0 ? { type: 'text', text: value, marks } : { type: 'text', text: value };
const paragraph = (...content: JSONContent[]): JSONContent => ({ type: 'paragraph', content });
const doc = (...content: JSONContent[]): JSONContent => ({ type: 'doc', content });
const bold = [{ type: 'bold' }];

function cell(
  type: 'tableCell' | 'tableHeader',
  content: JSONContent[],
  attrs: Record<string, unknown> = {},
): JSONContent {
  return { type, attrs: { colspan: 1, rowspan: 1, colwidth: null, align: null, ...attrs }, content };
}

function table(rows: JSONContent[][]): JSONContent {
  return { type: 'table', content: rows.map((cells) => ({ type: 'tableRow', content: cells })) };
}

function kinds(document: JSONContent): LossKind[] {
  return findLosses(document).map((loss) => loss.kind);
}

describe('findLosses', () => {
  it('finds nothing in a document Markdown can hold', () => {
    const header = [cell('tableHeader', [paragraph(text('A'))]), cell('tableHeader', [paragraph(text('B'))])];
    const body = [cell('tableCell', [paragraph(text('1'))]), cell('tableCell', [paragraph(text('2'))])];
    expect(
      findLosses(doc(paragraph(text('Ein '), text('fettes', bold), text(' Wort.')), table([header, body]))),
    ).toEqual([]);
  });

  it('names merged cells and several blocks in a cell (section 10)', () => {
    const merged = table([[cell('tableHeader', [paragraph(text('A'))], { colspan: 2 })]]);
    const blocks = table([[cell('tableHeader', [paragraph(text('A')), paragraph(text('B'))])]]);
    const list = table([
      [
        cell('tableHeader', [
          { type: 'bulletList', content: [{ type: 'listItem', content: [paragraph(text('x'))] }] },
        ]),
      ],
    ]);
    expect(kinds(doc(merged))).toEqual(['tableMergedCells']);
    expect(kinds(doc(blocks))).toEqual(['tableCellBlocks']);
    expect(kinds(doc(list))).toEqual(['tableCellBlocks']);
  });

  it('names a table without header row, header cells below it, alignment per cell and column widths', () => {
    expect(kinds(doc(table([[cell('tableCell', [paragraph(text('A'))])]])))).toEqual(['tableHeader']);
    const lowerHeader = table([
      [cell('tableHeader', [paragraph(text('A'))])],
      [cell('tableHeader', [paragraph(text('B'))])],
    ]);
    expect(kinds(doc(lowerHeader))).toEqual(['tableHeader']);
    const alignment = table([
      [cell('tableHeader', [paragraph(text('A'))], { align: 'left' })],
      [cell('tableCell', [paragraph(text('B'))], { align: 'right' })],
    ]);
    expect(kinds(doc(alignment))).toEqual(['tableAlignment']);
    const width = table([[cell('tableHeader', [paragraph(text('A'))], { colwidth: [120] })]]);
    expect(kinds(doc(width))).toEqual(['tableColumnWidth']);
  });

  it('names numbered lists with letters', () => {
    const list = {
      type: 'orderedList',
      attrs: { start: 1, type: 'a' },
      content: [{ type: 'listItem', content: [paragraph(text('x'))] }],
    };
    expect(kinds(doc(list))).toEqual(['listNumbering']);
  });

  it('names bold that ends at punctuation directly before a letter', () => {
    expect(kinds(doc(paragraph(text('Hinweis:', bold), text('Text'))))).toEqual(['emphasisBoundary']);
    expect(kinds(doc(paragraph(text('x'), text('(a)', bold))))).toEqual(['emphasisBoundary']);
    expect(
      kinds(doc(paragraph(text('x'), text('code', [{ type: 'bold' }, { type: 'code' }]), text('y')))),
    ).toEqual(['emphasisBoundary']);
    // A space or punctuation outside is fine, as is bold and italic together.
    expect(findLosses(doc(paragraph(text('Hinweis:', bold), text(' Text'))))).toEqual([]);
    expect(findLosses(doc(paragraph(text('„'), text('Wort', bold), text('“'))))).toEqual([]);
    expect(
      findLosses(doc(paragraph(text('x'), text('ab', [{ type: 'bold' }, { type: 'italic' }]), text('y')))),
    ).toEqual([]);
  });

  it('names inline code with backticks inside bold or a link', () => {
    const link = [{ type: 'link', attrs: { href: 'https://example.org' } }, { type: 'code' }];
    expect(kinds(doc(paragraph(text('a `b`', link))))).toEqual(['codeBackticks']);
    expect(
      kinds(doc(paragraph(text(' '), text('a `b`', [{ type: 'bold' }, { type: 'code' }]), text(' ')))),
    ).toEqual(['codeBackticks']);
    expect(findLosses(doc(paragraph(text('a `b`', [{ type: 'code' }]))))).toEqual([]);
  });

  it('names raw HTML at the start of a line', () => {
    const raw: JSONContent = { type: 'rawInline', attrs: { raw: '<span class="x">', kind: 'html' } };
    expect(kinds(doc(paragraph(raw, text(' Text'))))).toEqual(['htmlLineStart']);
    expect(kinds(doc(paragraph(text('a'), { type: 'hardBreak' }, raw)))).toEqual(['htmlLineStart']);
    expect(findLosses(doc(paragraph(text('a '), raw)))).toEqual([]);
  });

  it('reports the block and an excerpt', () => {
    const losses = findLosses(
      doc(paragraph(text('Davor')), paragraph(text('Hinweis:', bold), text('Text danach'))),
    );
    expect(losses).toEqual([{ kind: 'emphasisBoundary', block: 1, excerpt: 'Hinweis:Text danach' }]);
  });

  it('marks the losses Markdown would really have', () => {
    // What findLosses names really changes when saved and read again.
    const documents = [
      doc(paragraph(text('Hinweis:', bold), text('Text'))),
      doc(table([[cell('tableHeader', [paragraph(text('A')), paragraph(text('B'))])]])),
      doc(table([[cell('tableCell', [paragraph(text('A'))])]])),
    ];
    for (const document of documents) {
      const reread = parseMarkdown(serializeMarkdown({ frontMatter: '', body: document })).body;
      expect(JSON.stringify(reread)).not.toBe(JSON.stringify(document));
    }
  });
});

describe('probeMarkdown', () => {
  it('holds the cases of section 10 without loss', () => {
    const cases = [
      '---\ntitle: x\n---\n\n# H\n',
      'Text[^1].\n\n[^1]: Note.\n',
      '<div class="x">\nhi\n</div>\n\ntext <span>s</span>\n',
      '[![alt](img.png)](https://example.org)\n',
      '<img src="a.png" width="100">\n',
    ];
    for (const markdown of cases) {
      expect(probeMarkdown(markdown).issues, markdown).toEqual([]);
    }
  });

  it('opens a document in the source view when saving would change it', () => {
    // Found by the save-stability generator: marked reads this line
    // differently from CommonMark, and what it reads cannot be written back.
    expect(probeMarkdown('**`a`** ~~b\\😀 c~~\n').issues).toEqual([
      { kind: 'changed', block: 0, type: 'paragraph' },
    ]);
  });
});
