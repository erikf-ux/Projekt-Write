/**
 * Save stability (ADR 0014): for documents the editor can hold and the loss
 * check passes, saving as Markdown and reading the file again gives the same
 * document up to the allowed normalizations, and saving that again gives the
 * same file. Every document the loss check rejects is one Markdown cannot
 * write; the loss check must name every such document.
 *
 * The documents come from a seeded generator, so a failure is reproducible
 * from its number.
 */
import type { JSONContent } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import { lexBoth } from '@/test/plainMarked';
import { parseMarkdown, serializeMarkdown } from './adapter';
import { findLosses } from './losses';
import { firstDifference, markdownSchema } from './probe';

/** Mulberry32: small, fast, deterministic. */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

class Generator {
  private readonly next: () => number;

  constructor(seed: number) {
    this.next = random(seed);
  }

  integer(max: number): number {
    return Math.floor(this.next() * max);
  }

  chance(probability: number): boolean {
    return this.next() < probability;
  }

  pick<T>(values: readonly T[]): T {
    return values[this.integer(values.length)] as T;
  }
}

// Words and characters that mean something in Markdown, HTML or GFM.
const WORDS = [
  'Wort',
  'Größe',
  'Maß',
  '„Zitat“',
  'snake_case',
  '2*3',
  '_x_',
  '*y*',
  '**z**',
  '`c`',
  '~~s~~',
  '[a]',
  '[[Wiki]]',
  '[t](u)',
  '![i](p.png)',
  '[^1]',
  '[r]:',
  '# ',
  '#',
  '##',
  '> ',
  '- ',
  '+ ',
  '1. ',
  '1) ',
  '---',
  '===',
  '***',
  '|',
  '\\',
  '\\*',
  'C:\\Pfad',
  '&',
  '&amp;',
  '&copy;',
  '&#169;',
  '<',
  '<b>',
  '</i>',
  '<!-- c -->',
  '<u>',
  'a<br>b',
  '>',
  'https://example.org',
  'www.example.org',
  'mail@example.org',
  'ftp://x.org',
  ':',
  '.',
  '!',
  '(',
  ')',
  '{',
  '}',
  '\u00a0',
  '~',
  '^',
  '$a_1$',
  'tab\there',
  ' ',
  '  ',
  '\n',
  'x\n- y',
  'x\n# y',
  'x\n> y',
  'x\n1. y',
  'x\n```',
  'x\n---',
  'x\n===',
  'x\n| a |',
  'x\n[ ] y',
  '1.',
  '10.',
  'Dr. Müller',
  'a. b',
  '😀',
  'e\u0301',
  '\u200b',
] as const;

const MARK_SETS: JSONContent['marks'][] = [
  [],
  [],
  [],
  [{ type: 'bold' }],
  [{ type: 'italic' }],
  [{ type: 'bold' }, { type: 'italic' }],
  [{ type: 'strike' }],
  [{ type: 'underline' }],
  [{ type: 'code' }],
  [{ type: 'bold' }, { type: 'code' }],
  [{ type: 'link', attrs: { href: 'https://example.org/a_b?x=1&y=2' } }],
  [{ type: 'link', attrs: { href: 'https://example.org', title: 'Titel "q"' } }],
  [{ type: 'link', attrs: { href: 'https://example.org/(x)' } }, { type: 'italic' }],
  [{ type: 'link', attrs: { href: 'mailto:mail@example.org' } }, { type: 'code' }],
];

const INLINE_ATOMS: JSONContent[] = [
  { type: 'hardBreak' },
  { type: 'image', attrs: { src: 'bild.png', alt: 'Bild [1]' } },
  { type: 'image', attrs: { src: 'a b.png', alt: '', title: 'T' } },
  { type: 'image', attrs: { src: 'w.png', alt: 'W', width: '100' } },
  { type: 'image', attrs: { src: 'l.png', alt: 'L', href: 'https://example.org' } },
  { type: 'rawInline', attrs: { raw: '<span class="x">', kind: 'html' } },
  { type: 'rawInline', attrs: { raw: '<!-- note -->', kind: 'html' } },
  { type: 'rawInline', attrs: { raw: '[^1]', kind: 'footnote' } },
];

function text(generator: Generator): string {
  const count = 1 + generator.integer(4);
  let value = '';
  for (let index = 0; index < count; index += 1) {
    value += generator.pick(WORDS);
    if (generator.chance(0.6)) value += ' ';
  }
  return value;
}

function inline(generator: Generator, allowBreaks: boolean): JSONContent[] {
  const content: JSONContent[] = [];
  const count = 1 + generator.integer(5);
  for (let index = 0; index < count; index += 1) {
    if (generator.chance(0.15)) {
      const atom = generator.pick(INLINE_ATOMS);
      if (atom.type !== 'hardBreak' || allowBreaks) content.push(atom);
      continue;
    }
    const marks = generator.pick(MARK_SETS);
    if (marks !== undefined && marks.length > 0) {
      // Formatting starts and ends at a letter and stands apart from its
      // neighbours: a boundary at punctuation is a loss (`losses.test.ts`),
      // and marked reads some runs like `**`x`**~~` differently from
      // CommonMark (the save check catches those).
      content.push(
        { type: 'text', text: ' ' },
        { type: 'text', text: `x${text(generator)}x`, marks },
        { type: 'text', text: ' ' },
      );
    } else {
      content.push({ type: 'text', text: text(generator) });
    }
  }
  return content;
}

function paragraph(generator: Generator): JSONContent {
  return generator.chance(0.05)
    ? { type: 'paragraph' }
    : { type: 'paragraph', content: inline(generator, true) };
}

function listItem(generator: Generator, type: 'listItem' | 'taskItem', depth: number): JSONContent {
  const content: JSONContent[] = [paragraph(generator)];
  if (depth < 2 && generator.chance(0.3)) content.push(block(generator, depth + 1));
  const item: JSONContent = { type, content };
  if (type === 'taskItem') item.attrs = { checked: generator.chance(0.5) };
  return item;
}

function list(generator: Generator, depth: number): JSONContent {
  const kind = generator.pick(['bulletList', 'orderedList', 'taskList'] as const);
  const itemType = kind === 'taskList' ? 'taskItem' : 'listItem';
  const items = Array.from({ length: 1 + generator.integer(3) }, () => listItem(generator, itemType, depth));
  const node: JSONContent = { type: kind, content: items };
  if (kind === 'orderedList' && generator.chance(0.3)) node.attrs = { start: 1 + generator.integer(20) };
  return node;
}

function table(generator: Generator): JSONContent {
  const columns = 1 + generator.integer(3);
  const cell = (type: string): JSONContent => ({
    type,
    content: [{ type: 'paragraph', content: generator.chance(0.1) ? [] : inline(generator, true) }],
  });
  const row = (type: string): JSONContent => ({
    type: 'tableRow',
    content: Array.from({ length: columns }, () => cell(type)),
  });
  return {
    type: 'table',
    content: [
      row('tableHeader'),
      ...Array.from({ length: 1 + generator.integer(2) }, () => row('tableCell')),
    ],
  };
}

const CODE = [
  'const a = 1;',
  '```',
  '````',
  '~~~',
  '  indented',
  '',
  '\t tab',
  '<div>',
  '# no heading',
  'end ',
];
const RAW_BLOCKS = [
  '<div class="box">\nText\n</div>',
  '<!-- comment -->',
  '<details>\n<summary>S</summary>\nInhalt\n</details>',
  '<pre>\na\n\n\nb\n</pre>',
  '[^1]: Fußnote.',
];

function block(generator: Generator, depth: number): JSONContent {
  const kind = generator.integer(10);
  switch (kind) {
    case 0:
      return {
        type: 'heading',
        attrs: { level: 1 + generator.integer(6) },
        content: inline(generator, true),
      };
    case 1:
      return depth < 2
        ? { type: 'blockquote', content: [block(generator, depth + 1)] }
        : paragraph(generator);
    case 2:
      return depth < 2 ? list(generator, depth) : paragraph(generator);
    case 3: {
      const lines = Array.from({ length: 1 + generator.integer(3) }, () => generator.pick(CODE));
      const language = generator.pick([null, 'js', 'rust']);
      return {
        type: 'codeBlock',
        attrs: { language },
        content: [{ type: 'text', text: lines.join('\n') }].filter((node) => node.text !== ''),
      };
    }
    case 4:
      return { type: 'horizontalRule' };
    case 5:
      return depth === 0 ? table(generator) : paragraph(generator);
    case 6: {
      const raw = generator.pick(RAW_BLOCKS);
      return { type: 'rawBlock', attrs: { raw, kind: raw.startsWith('[^') ? 'footnote' : 'html' } };
    }
    default:
      return paragraph(generator);
  }
}

export function documentForSeed(seed: number): JSONContent {
  const generator = new Generator(seed);
  return {
    type: 'doc',
    content: Array.from({ length: 1 + generator.integer(6) }, () => block(generator, 0)),
  };
}

/** A document as the editor holds it: checked by the schema, with all attributes. */
function asEditorJson(json: JSONContent): JSONContent {
  const node = markdownSchema().nodeFromJSON(json);
  node.check();
  return node.toJSON() as JSONContent;
}

const SEEDS = Array.from({ length: 300 }, (_, index) => index + 1);

describe('save stability', () => {
  it('the generator mostly produces documents without losses', () => {
    const lossless = SEEDS.filter((seed) => findLosses(asEditorJson(documentForSeed(seed))).length === 0);
    expect(lossless.length).toBeGreaterThan(SEEDS.length / 2);
  });

  it.each(SEEDS)('document %i', (seed) => {
    const original = asEditorJson(documentForSeed(seed));
    const markdown = serializeMarkdown({ frontMatter: '', body: original });
    // The quick checks in front of marked's rules (manager.ts) read the same tokens.
    const lexed = lexBoth(markdown);
    expect(lexed.checked).toBe(lexed.plain);
    if (findLosses(original).length > 0) {
      return;
    }
    const reread = parseMarkdown(markdown).body;
    const difference = firstDifference(original, asEditorJson(reread));
    expect(difference, `${markdown}\n${JSON.stringify(original)}`).toBeNull();
    expect(serializeMarkdown({ frontMatter: '', body: reread })).toBe(markdown);
  });
});
