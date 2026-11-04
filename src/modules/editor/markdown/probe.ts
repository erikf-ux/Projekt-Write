/**
 * Round-trip probe when a Markdown document is opened (section 10: "Der
 * Adapter prüft schon beim Öffnen per Round-Trip-Probe, ob er ein
 * Markdown-Dokument verlustfrei halten kann. Kann er es nicht, öffnet es in
 * der Quelltextansicht und nennt den Grund.").
 *
 * 1. Every Markdown token was held by an extension of the schema.
 * 2. The document fits the schema (ProseMirror's own check).
 * 3. Saving and reading again gives the same document up to the allowed
 *    normalizations; `canonicalBlocks` is their executable definition
 *    (ADR 0014). The comparison is on the document, not on rendered HTML,
 *    and the YAML front matter must come back byte for byte. If saving
 *    writes the file exactly as it is (a file the app saved), reading it
 *    again gives the same document, so the second reading is skipped.
 */
import { getSchema, type JSONContent } from '@tiptap/core';
import type { Schema } from '@tiptap/pm/model';
import { markdownExtensions } from '../schema/extensions';
import {
  isBlankParagraph,
  isNbspParagraph,
  markdownManager,
  parseMarkdown,
  serializeMarkdown,
  type MarkdownDocument,
} from './adapter';

export type ProbeIssue =
  /** Markdown the schema has no place for, by marked token type. */
  | { kind: 'unsupported'; tokens: string[] }
  /** The parsed document breaks a rule of the schema. */
  | { kind: 'invalid' }
  /** Saving would change this top-level block (0-based). */
  | { kind: 'changed'; block: number; type: string }
  | { kind: 'frontMatter' };

export interface ProbeResult {
  document: MarkdownDocument;
  issues: ProbeIssue[];
}

let schema: Schema | null = null;

export function markdownSchema(): Schema {
  schema ??= getSchema(markdownExtensions());
  return schema;
}

export function probeMarkdown(markdown: string): ProbeResult {
  const document = parseMarkdown(markdown);
  const issues: ProbeIssue[] = [];
  const unsupported = [...markdownManager().unsupported].sort();
  if (unsupported.length > 0) {
    issues.push({ kind: 'unsupported', tokens: unsupported });
  }
  try {
    markdownSchema().nodeFromJSON(document.body).check();
  } catch {
    issues.push({ kind: 'invalid' });
    return { document, issues };
  }
  const saved = serializeMarkdown(document);
  if (saved === markdown) {
    return { document, issues };
  }
  const again = parseMarkdown(saved);
  if (again.frontMatter !== document.frontMatter) {
    issues.push({ kind: 'frontMatter' });
  }
  const change = firstDifference(document.body, again.body);
  if (change !== null) {
    issues.push({ kind: 'changed', ...change });
  }
  return { document, issues };
}

/** The first top-level block that differs after the allowed normalizations. */
export function firstDifference(
  before: JSONContent,
  after: JSONContent,
): { block: number; type: string } | null {
  const left = withoutBlankEdges(before.content ?? []);
  const right = withoutBlankEdges(after.content ?? []);
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const a = left[index];
    const b = right[index];
    // Most blocks come back exactly; only the others need the canonical form.
    if (a !== undefined && b !== undefined && JSON.stringify(a) === JSON.stringify(b)) {
      continue;
    }
    if (a === undefined || b === undefined || canonicalKey(a) !== canonicalKey(b)) {
      return { block: index, type: a?.type ?? b?.type ?? 'doc' };
    }
  }
  return null;
}

function withoutBlankEdges(blocks: JSONContent[]): JSONContent[] {
  let start = 0;
  let end = blocks.length;
  while (start < end && isBlankParagraph(blocks[start])) start += 1;
  while (end > start && isBlankParagraph(blocks[end - 1])) end -= 1;
  return blocks.slice(start, end);
}

function canonicalKey(block: JSONContent): string {
  return JSON.stringify(canonicalNode(block, false));
}

interface CanonicalBlock {
  type: string;
  key: string;
}

/**
 * The document in canonical form, one entry per top-level block. Allowed
 * normalizations (ADR 0014):
 *
 * * Paragraphs without text (empty, only whitespace or hard breaks) at the
 *   start and end of the document are dropped.
 * * Whitespace at the edges of paragraphs, headings and table cells, and
 *   hard breaks at their edges are dropped.
 * * Whitespace before and after a line break is dropped; in a heading a
 *   line break in the text becomes a space; in a table cell whitespace runs
 *   become one space.
 * * Whitespace at the boundary of a formatting keeps only the formatting of
 *   both neighbours (Markdown cannot open `**` before a space); whitespace
 *   at the edge of inline code is outside the code.
 * * A line break inside inline code is a space.
 * * Mark order and attributes Markdown does not write are ignored.
 */
export function canonicalBlocks(document: JSONContent): CanonicalBlock[] {
  return withoutBlankEdges(document.content ?? []).map((block) => ({
    type: block.type ?? 'unknown',
    key: canonicalKey(block),
  }));
}

/** Attributes Markdown writes, with a stable key order. */
function canonicalAttributes(
  attrs: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (attrs === undefined) {
    return undefined;
  }
  const entries = Object.entries(attrs)
    .filter(([name, value]) => value !== null && value !== undefined && name !== 'pwMarker')
    .sort(([a], [b]) => a.localeCompare(b));
  return entries.length === 0 ? undefined : Object.fromEntries(entries);
}

function canonicalNode(node: JSONContent, inTable: boolean): unknown {
  const type = node.type ?? 'unknown';
  const attrs = canonicalAttributes(node.attrs);
  if (type === 'text') {
    return { type, text: node.text ?? '', marks: markKey(node.marks) };
  }
  if (type === 'paragraph' || type === 'heading') {
    // Outside tables `&nbsp;` alone is how an empty paragraph is written.
    const content = !inTable && isNbspParagraph(node) ? [] : (node.content ?? []);
    return { type, attrs, content: canonicalInline(content, type === 'heading', inTable) };
  }
  const cell = inTable || type === 'tableCell' || type === 'tableHeader';
  return { type, attrs, content: (node.content ?? []).map((child) => canonicalNode(child, cell)) };
}

function markKey(marks: JSONContent['marks'], node = -1): string {
  return (marks ?? [])
    .map((mark) => {
      const href: unknown = mark.attrs?.['href'];
      const title: unknown = mark.attrs?.['title'];
      const attrs = mark.type === 'link' ? { href: href ?? '', title: title ?? null } : null;
      // Each code span is written on its own (adapter: `pwRun`), so whitespace
      // at its edge leaves it, also next to another code span.
      const key = JSON.stringify([mark.type, attrs]);
      return mark.type === 'code' && node >= 0 ? `${key}#${String(node)}` : key;
    })
    .sort()
    .join('');
}

/**
 * `loose`: whitespace Tiptap moves out of a formatting at its edge (any
 * Unicode space; a non-breaking space only in code, elsewhere it is written
 * as `&nbsp;`).
 */
type Piece = { text: string; marks: string; loose: boolean } | { atom: string; marks: string };

const WHITESPACE = /^[ \t\n]$/;

function isBreak(piece: Piece | undefined): boolean {
  return piece !== undefined && 'atom' in piece && piece.atom.startsWith('{"type":"hardBreak"');
}

function isSpace(piece: Piece | undefined): boolean {
  return piece !== undefined && 'text' in piece && WHITESPACE.test(piece.text);
}

function isLoose(piece: Piece | undefined): boolean {
  return piece !== undefined && 'text' in piece && piece.loose;
}

const MARK = /\[[^\]]*\](?:#\d+)?/g;

/** Marks two mark keys have in common. */
function commonMarks(a: string, b: string): string {
  const left = new Set(a.match(MARK) ?? []);
  return (b.match(MARK) ?? []).filter((mark) => left.has(mark)).join('');
}

function canonicalInline(content: JSONContent[], heading: boolean, inTable: boolean): unknown[] {
  // One piece per character or atom.
  let pieces: Piece[] = [];
  content.forEach((child, index) => {
    const marks = markKey(child.marks, index);
    if (child.type === 'text') {
      const code = (child.marks ?? []).some((mark) => mark.type === 'code');
      // A code span has no line breaks (CommonMark reads them as spaces).
      for (const character of code ? (child.text ?? '').replace(/\n/g, ' ') : (child.text ?? '')) {
        pieces.push({
          text: character,
          marks,
          loose: /^\s$/.test(character) && (code || character !== '\u00a0'),
        });
      }
    } else {
      pieces.push({ atom: JSON.stringify(canonicalNode({ ...child, marks: [] }, inTable)), marks });
    }
  });
  // Whitespace keeps only the marks of both neighbours.
  const left: string[] = [];
  let last: string | null = null;
  for (const piece of pieces) {
    left.push(last ?? '');
    if (!isLoose(piece)) last = piece.marks;
  }
  let next: string | null = null;
  for (let index = pieces.length - 1; index >= 0; index -= 1) {
    const piece = pieces[index] as Piece;
    if (isLoose(piece) && 'text' in piece) {
      pieces[index] = {
        ...piece,
        marks: commonMarks(commonMarks(piece.marks, left[index] ?? ''), next ?? ''),
      };
    } else {
      next = piece.marks;
    }
  }
  pieces = pieces.map((piece) => ({ ...piece, marks: piece.marks.replace(/#\d+/g, '') }));
  // Edges: no whitespace, no hard break.
  while (isSpace(pieces[0]) || isBreak(pieces[0])) pieces.shift();
  while (isSpace(pieces[pieces.length - 1]) || isBreak(pieces[pieces.length - 1])) pieces.pop();
  // A line break next to a hard break goes, several line breaks are one.
  pieces = pieces.filter((piece, index) => {
    if (!('text' in piece) || piece.text !== '\n') {
      return true;
    }
    const neighbour = (step: number): Piece | undefined => {
      let at = index + step;
      while (
        isSpace(pieces[at]) &&
        !('text' in (pieces[at] as Piece) && (pieces[at] as { text: string }).text === '\n')
      )
        at += step;
      return pieces[at];
    };
    const previous = neighbour(-1);
    const following = neighbour(1);
    const lineBreak = (candidate: Piece | undefined): boolean =>
      candidate !== undefined && 'text' in candidate && candidate.text === '\n';
    return !(isBreak(previous) || isBreak(following) || lineBreak(following));
  });
  // Whitespace after a line break or hard break goes (CommonMark ignores it).
  pieces = pieces.filter((piece, index) => {
    if (!isSpace(piece) || ('text' in piece && piece.text === '\n')) {
      return true;
    }
    let before = index - 1;
    while (
      isSpace(pieces[before]) &&
      'text' in (pieces[before] as Piece) &&
      (pieces[before] as { text: string }).text !== '\n'
    ) {
      before -= 1;
    }
    const preceding = pieces[before];
    return !(
      isBreak(preceding) ||
      (preceding !== undefined && 'text' in preceding && preceding.text === '\n')
    );
  });
  // Whitespace before a line break or hard break goes.
  pieces = pieces.filter((piece, index) => {
    if (!isSpace(piece) || ('text' in piece && piece.text === '\n')) {
      return true;
    }
    let after = index + 1;
    while (
      isSpace(pieces[after]) &&
      'text' in (pieces[after] as Piece) &&
      (pieces[after] as { text: string }).text !== '\n'
    ) {
      after += 1;
    }
    const following = pieces[after];
    return !(
      isBreak(following) ||
      (following !== undefined && 'text' in following && following.text === '\n')
    );
  });
  if (heading || inTable) {
    pieces = collapseWhitespace(pieces, heading);
    while (isSpace(pieces[0])) pieces.shift();
    while (isSpace(pieces[pieces.length - 1])) pieces.pop();
  }
  return mergePieces(pieces).map((piece) =>
    'text' in piece ? { text: piece.text, marks: piece.marks } : piece,
  );
}

/**
 * Heading: whitespace around a line break becomes one space (an ATX heading
 * is one line). Table cell: every whitespace run becomes one space.
 */
function collapseWhitespace(pieces: Piece[], heading: boolean): Piece[] {
  const result: Piece[] = [];
  let index = 0;
  while (index < pieces.length) {
    const piece = pieces[index] as Piece;
    const blank = (candidate: Piece | undefined): boolean =>
      isSpace(candidate) || (!heading && isLoose(candidate));
    if (!blank(piece)) {
      result.push(piece);
      index += 1;
      continue;
    }
    let end = index;
    while (end < pieces.length && blank(pieces[end])) end += 1;
    const run = pieces.slice(index, end);
    if (!heading || run.some((candidate) => 'text' in candidate && candidate.text === '\n')) {
      const marks = run.map((candidate) => candidate.marks).reduce((a, b) => commonMarks(a, b));
      result.push({ text: ' ', marks, loose: true });
    } else {
      result.push(...run);
    }
    index = end;
  }
  return result;
}

function mergePieces(pieces: Piece[]): Piece[] {
  const result: Piece[] = [];
  for (const piece of pieces) {
    const previous = result[result.length - 1];
    if (previous !== undefined && 'text' in previous && 'text' in piece && previous.marks === piece.marks) {
      result[result.length - 1] = { text: previous.text + piece.text, marks: piece.marks, loose: false };
    } else {
      result.push(piece);
    }
  }
  return result;
}
