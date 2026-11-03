/**
 * Loss check before saving as Markdown (section 10: "Würde beim Speichern
 * als MD oder TXT etwas verloren gehen, warne vorher und nenne die
 * betroffenen Inhalte.").
 *
 * The schema holds only what Markdown can hold, with these exceptions, which
 * the editor can produce but Markdown cannot write (ADR 0014):
 *
 * * Tables: merged cells, several blocks or a non-paragraph in a cell, no
 *   header row or header cells below it, alignment per cell instead of per
 *   column, fixed column widths.
 * * Numbered lists with letters or Roman numerals.
 * * Bold or italic that starts or ends at punctuation next to a letter
 *   (`**Hinweis:**Text`): CommonMark reads the `**` as text there.
 * * Inline code with backticks in it that is also bold, italic or a link:
 *   marked does not find the formatting around it.
 * * Raw HTML at the start of a line, where Markdown reads it as an HTML
 *   block, and a sized image alone on a line inside a list item.
 */
import type { JSONContent } from '@tiptap/core';

export type LossKind =
  | 'tableMergedCells'
  | 'tableCellBlocks'
  | 'tableHeader'
  | 'tableAlignment'
  | 'tableColumnWidth'
  | 'listNumbering'
  | 'emphasisBoundary'
  | 'codeBackticks'
  | 'htmlLineStart';

export interface Loss {
  kind: LossKind;
  /** Index of the top-level block (0-based). */
  block: number;
  /** A short piece of the text concerned, for the warning. */
  excerpt: string;
}

const EXCERPT_LENGTH = 40;

function textOf(node: JSONContent): string {
  if (node.type === 'text') {
    return node.text ?? '';
  }
  return (node.content ?? []).map(textOf).join(node.type === 'paragraph' ? '' : ' ');
}

function excerpt(node: JSONContent): string {
  const text = textOf(node).replace(/\s+/g, ' ').trim();
  return text.length > EXCERPT_LENGTH ? `${text.slice(0, EXCERPT_LENGTH - 1)}…` : text;
}

/** All losses of a document, in document order, at most one per kind and block. */
export function findLosses(document: JSONContent): Loss[] {
  const losses: Loss[] = [];
  (document.content ?? []).forEach((block, index) => {
    const found = new Map<LossKind, string>();
    const report = (kind: LossKind, node: JSONContent): void => {
      if (!found.has(kind)) found.set(kind, excerpt(node));
    };
    visit(block, report, { inList: false });
    for (const [kind, text] of found) {
      losses.push({ kind, block: index, excerpt: text });
    }
  });
  return losses;
}

interface Context {
  inList: boolean;
}

type Report = (kind: LossKind, node: JSONContent) => void;

function visit(node: JSONContent, report: Report, context: Context): void {
  switch (node.type) {
    case 'table':
      checkTable(node, report);
      break;
    case 'orderedList': {
      const type: unknown = node.attrs?.['type'];
      if (typeof type === 'string' && type !== '' && type !== '1') report('listNumbering', node);
      break;
    }
    case 'paragraph':
    case 'heading':
      checkInline(node, report, context);
      return;
    default:
      break;
  }
  const inList = context.inList || node.type === 'listItem' || node.type === 'taskItem';
  for (const child of node.content ?? []) {
    visit(child, report, { inList });
  }
}

function checkTable(table: JSONContent, report: Report): void {
  const rows = table.content ?? [];
  const alignments = new Map<number, unknown>();
  rows.forEach((row, rowIndex) => {
    (row.content ?? []).forEach((cell, column) => {
      const attrs = (cell.attrs ?? {}) as Record<string, unknown>;
      if (Number(attrs['colspan'] ?? 1) > 1 || Number(attrs['rowspan'] ?? 1) > 1)
        report('tableMergedCells', cell);
      const blocks = cell.content ?? [];
      if (blocks.length > 1 || blocks.some((block) => block.type !== 'paragraph'))
        report('tableCellBlocks', cell);
      if ((cell.type === 'tableHeader') !== (rowIndex === 0)) report('tableHeader', cell);
      const width = attrs['colwidth'];
      if (Array.isArray(width) && width.some((value) => value !== null && value !== 0))
        report('tableColumnWidth', cell);
      const align = attrs['align'] ?? null;
      if (alignments.has(column) && alignments.get(column) !== align) report('tableAlignment', cell);
      if (!alignments.has(column)) alignments.set(column, align);
    });
  });
}

/** Marks written with `*` delimiters, which follow the CommonMark flanking rules. */
const DELIMITER_MARKS = new Set(['bold', 'italic']);
/**
 * Marks with a syntax of their own, outermost first (the order in which the
 * serializer nests them): `[…](…)`, `**`, `*`, `~~`, `<u>`, `` ` ``.
 */
const NESTING = ['link', 'bold', 'italic', 'strike', 'underline', 'code'];

interface Slot {
  /** The character as Markdown sees it next to a delimiter. */
  kind: 'space' | 'punctuation' | 'other' | 'edge';
  marks: Set<string>;
}

const PUNCTUATION = /[\p{P}\p{S}]/u;

function classify(character: string, code: boolean): Slot['kind'] {
  // Outside code a non-breaking space is written as `&nbsp;`.
  if (character === ' ' && !code) return 'punctuation';
  if (/\s/.test(character)) return 'space';
  return PUNCTUATION.test(character) ? 'punctuation' : 'other';
}

function checkInline(node: JSONContent, report: Report, context: Context): void {
  const slots: Slot[] = [{ kind: 'edge', marks: new Set() }];
  let lineStart = true;
  for (const child of node.content ?? []) {
    const marks = new Set((child.marks ?? []).map((mark) => mark.type));
    if (child.type === 'text') {
      const text = child.text ?? '';
      const code = marks.has('code');
      if (code && text.includes('`') && (marks.has('link') || marks.has('bold') || marks.has('italic'))) {
        report('codeBackticks', node);
      }
      for (const character of text) {
        slots.push({ kind: classify(character, code), marks });
        lineStart = character === '\n';
      }
      continue;
    }
    if (child.type === 'hardBreak') {
      slots.push({ kind: 'space', marks });
      lineStart = true;
      continue;
    }
    const html = child.type === 'rawInline' && child.attrs?.['kind'] === 'html';
    const sizedImage =
      child.type === 'image' && (child.attrs?.['width'] != null || child.attrs?.['height'] != null);
    if (lineStart && (html || (sizedImage && context.inList))) report('htmlLineStart', node);
    // Atoms start with `!`, `<` or `[` in Markdown.
    slots.push({ kind: 'punctuation', marks });
    lineStart = false;
  }
  slots.push({ kind: 'edge', marks: new Set() });
  if (hasUnreadableDelimiter(slots)) {
    report('emphasisBoundary', node);
  }
}

/**
 * Bold and italic open where a mark starts and close where it ends. Next to
 * their `*` run is either the text or the syntax of another mark that opens
 * or closes at the same place (punctuation); bold and italic together form
 * one run. CommonMark: a run of punctuation-side `*` next to a letter is not
 * flanking, so `**Hinweis:**Text` stays text.
 */
function hasUnreadableDelimiter(slots: Slot[]): boolean {
  for (let index = 1; index < slots.length; index += 1) {
    const before = slots[index - 1] as Slot;
    const after = slots[index] as Slot;
    const opening = NESTING.filter((mark) => !before.marks.has(mark) && after.marks.has(mark));
    const closing = NESTING.filter((mark) => before.marks.has(mark) && !after.marks.has(mark));
    const syntax = (marks: string[], test: (rank: number) => boolean): boolean =>
      marks.some((mark) => !DELIMITER_MARKS.has(mark) && test(NESTING.indexOf(mark)));
    // Bold and italic changing together form one `*` run with the same neighbours.
    for (const mark of opening.filter((name) => DELIMITER_MARKS.has(name))) {
      const rank = NESTING.indexOf(mark);
      const outside =
        syntax(closing, () => true) || syntax(opening, (other) => other < rank) ? 'punctuation' : before.kind;
      const inside = syntax(opening, (other) => other > rank) ? 'punctuation' : after.kind;
      if (inside === 'punctuation' && outside === 'other') return true;
    }
    for (const mark of closing.filter((name) => DELIMITER_MARKS.has(name))) {
      const rank = NESTING.indexOf(mark);
      const inside = syntax(closing, (other) => other > rank) ? 'punctuation' : before.kind;
      const outside =
        syntax(opening, () => true) || syntax(closing, (other) => other < rank) ? 'punctuation' : after.kind;
      if (inside === 'punctuation' && outside === 'other') return true;
    }
  }
  return false;
}
