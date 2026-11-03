/**
 * The Markdown adapter (section 7: `@tiptap/markdown` "läuft deshalb hinter
 * einer eigenen Adapter-Schicht"). Everything that reads or writes Markdown
 * goes through here (ADR 0014).
 *
 * * A YAML front matter is cut off before parsing and put back unchanged
 *   (section 10).
 * * Blank lines between blocks are layout, not content: several blank lines
 *   do not become empty paragraphs (Tiptap would create them).
 * * Tabs that are content survive list items (marked expands them).
 * * Before writing, leading and trailing empty paragraphs are dropped and
 *   adjacent lists get different markers.
 */
import type { JSONContent } from '@tiptap/core';
import { markdownExtensions } from '../schema/extensions';
import { ProjectWriteMarkdownManager } from './manager';

export interface MarkdownDocument {
  /** The YAML front matter with its delimiters and the blank lines after it. */
  frontMatter: string;
  body: JSONContent;
}

const FRONT_MATTER = /^---[ \t]*\n[\s\S]*?\n(?:---|\.\.\.)[ \t]*(?:\n|$)(?:[ \t]*\n)*/;

let manager: ProjectWriteMarkdownManager | null = null;

export function markdownManager(): ProjectWriteMarkdownManager {
  manager ??= new ProjectWriteMarkdownManager({ extensions: markdownExtensions() });
  return manager;
}

/** Splits off a YAML front matter at the very start of the file. */
export function splitFrontMatter(markdown: string): { frontMatter: string; body: string } {
  const match = FRONT_MATTER.exec(markdown);
  return match
    ? { frontMatter: match[0], body: markdown.slice(match[0].length) }
    : { frontMatter: '', body: markdown };
}

export function parseMarkdown(markdown: string): MarkdownDocument {
  const { frontMatter, body } = splitFrontMatter(markdown.replace(/\r\n?/g, '\n'));
  const protectedBody = protectTabs(body);
  const parsed = markdownManager().parse(protectedBody);
  const normalized = normalizeParsed(protectedBody === body ? parsed : restoreTabs(parsed));
  // An empty file is one empty paragraph (the schema wants a block).
  const blocks = normalized.content ?? [];
  const content = blocks.length > 0 ? blocks : [{ type: 'paragraph', content: [] }];
  return { frontMatter, body: { ...normalized, content } };
}

/** A paragraph holding only a non-breaking space (`&nbsp;`) is an empty paragraph, outside tables. */
function normalizeParsed(node: JSONContent): JSONContent {
  if (!Array.isArray(node.content) || node.type === 'table') {
    return node;
  }
  if (isNbspParagraph(node)) {
    return { ...node, content: [] };
  }
  return { ...node, content: node.content.map(normalizeParsed) };
}

export function isNbspParagraph(node: JSONContent): boolean {
  const [only] = node.content ?? [];
  return (
    node.type === 'paragraph' && node.content?.length === 1 && only?.type === 'text' && only.text === '\u00a0'
  );
}

/** Stands for a tab while marked reads the text (Unicode private use). */
const TAB_MARK = '\uE000';

/** A code fence, also after list markers or `>` (`- ```js`). */
const FENCE_LINE = /^((?:[ \t]*(?:>[ \t]?|(?:[-+*]|\d{1,9}[.)])[ \t]+))*[ \t]*)(`{3,}|~{3,})(.*)$/;

/** Markers at the start of a line whose following tab is structure. */
const LINE_PREFIX = /^[ \t]*(?:(?:>|[-+*]|\d{1,9}[.)]|#{1,6}|\[[ xX]\])(?:[ \t]+|$))*/;

/**
 * marked turns tabs inside list items into spaces, also in code. Tabs that
 * are content are hidden from it and restored after parsing: inside a fenced
 * code block everything after the fence's indentation, elsewhere tabs after
 * the line's block markers and before its last character.
 */
export function protectTabs(markdown: string): string {
  if (!markdown.includes('\t') || markdown.includes(TAB_MARK)) {
    return markdown;
  }
  let fence: { indent: string; marker: string } | null = null;
  return markdown
    .split('\n')
    .map((line) => {
      const match = FENCE_LINE.exec(line);
      const marker = match?.[2];
      if (fence === null) {
        if (match !== null && marker !== undefined) {
          // Lines of the code block start at the fence's column.
          fence = {
            indent: (match[1] ?? '').replace(/[-+*]|\d{1,9}[.)]/g, (item) => ' '.repeat(item.length)),
            marker,
          };
          return line;
        }
        return protectContentTabs(line);
      }
      if (
        marker !== undefined &&
        marker.startsWith(fence.marker.charAt(0)) &&
        marker.length >= fence.marker.length &&
        (match?.[3] ?? '').trim() === ''
      ) {
        fence = null;
        return line;
      }
      return line.startsWith(fence.indent)
        ? fence.indent + line.slice(fence.indent.length).replace(/\t/g, TAB_MARK)
        : line;
    })
    .join('\n');
}

function protectContentTabs(line: string): string {
  const start = LINE_PREFIX.exec(line)?.[0].length ?? 0;
  const end = line.search(/[ \t]*$/);
  if (end <= start) {
    return line;
  }
  return line.slice(0, start) + line.slice(start, end).replace(/\t/g, TAB_MARK) + line.slice(end);
}

function restoreTabs<T>(value: T): T {
  if (typeof value === 'string') {
    return value.replaceAll(TAB_MARK, '\t') as T;
  }
  if (Array.isArray(value)) {
    return value.map((item: unknown) => restoreTabs(item)) as T;
  }
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, restoreTabs(item)])) as T;
  }
  return value;
}

export function serializeMarkdown(document: MarkdownDocument): string {
  let body = markdownManager().serialize(prepareForMarkdown(document.body));
  // A thematic break at the start must not look like a front matter.
  if (document.frontMatter === '' && FRONT_MATTER.test(body)) {
    body = `***${body.slice(3)}`;
  }
  return document.frontMatter + (body === '' ? '' : `${body}\n`);
}

type ListKind = 'bullet' | 'ordered';

function listKind(type: string | undefined): ListKind | null {
  if (type === 'bulletList' || type === 'taskList') {
    return 'bullet';
  }
  return type === 'orderedList' ? 'ordered' : null;
}

const DEFAULT_MARKER: Record<ListKind, string> = { bullet: '-', ordered: '.' };
const OTHER_MARKER: Record<ListKind, string> = { bullet: '*', ordered: ')' };

/** A paragraph without text: empty, only whitespace or hard breaks. */
export function isBlankParagraph(node: JSONContent | undefined): boolean {
  return (
    node?.type === 'paragraph' &&
    (node.content ?? []).every(
      (child) => child.type === 'hardBreak' || (child.type === 'text' && /^[ \t\n]*$/.test(child.text ?? '')),
    )
  );
}

/**
 * A copy of the document as Markdown writes it: without empty paragraphs at
 * the edges, with alternating markers for adjacent lists of the same kind,
 * numbered lists with digits, each code span on its own, autolinks raw.
 */
export function prepareForMarkdown(document: JSONContent): JSONContent {
  const content = prepareContent(document.content ?? [], { codeRuns: 0 });
  let start = 0;
  let end = content.length;
  while (start < end && isBlankParagraph(content[start])) start += 1;
  while (end > start && isBlankParagraph(content[end - 1])) end -= 1;
  return { ...document, content: content.slice(start, end) };
}

interface PrepareState {
  codeRuns: number;
}

/**
 * Whitespace at the edges of inline code goes outside the code, as its own
 * text: Markdown writes it there anyway, and outside code a non-breaking
 * space is written as `&nbsp;`.
 */
function splitCodeEdges(content: JSONContent[]): JSONContent[] {
  return content.flatMap((child) => {
    if (child.type !== 'text' || child.marks?.some((mark) => mark.type === 'code') !== true) {
      return [child];
    }
    const match = /^(\s*)([\s\S]*?)(\s*)$/.exec(child.text ?? '');
    const [, leading = '', code = '', trailing = ''] = match ?? [];
    if (leading === '' && trailing === '') {
      return [child];
    }
    const outside = child.marks.filter((mark) => mark.type !== 'code');
    const plain = (text: string): JSONContent[] =>
      text === ''
        ? []
        : [outside.length > 0 ? { type: 'text', text, marks: outside } : { type: 'text', text }];
    return [...plain(leading), ...(code === '' ? [] : [{ ...child, text: code }]), ...plain(trailing)];
  });
}

const AUTOLINK_URI = /^[A-Za-z][A-Za-z0-9+.-]{1,31}:[^\s<>\p{Cc}]*$/u;
const AUTOLINK_EMAIL =
  /^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$/;

/**
 * A link whose text is its address, with no other formatting, as `<…>`:
 * written raw, because nothing inside an autolink is escaped.
 */
function asAutolink(
  node: JSONContent,
  previous: JSONContent | undefined,
  next: JSONContent | undefined,
): JSONContent | null {
  const [mark, ...others] = node.marks ?? [];
  if (node.type !== 'text' || mark?.type !== 'link' || others.length > 0) {
    return null;
  }
  const href: unknown = mark.attrs?.['href'];
  const title: unknown = mark.attrs?.['title'];
  const text = node.text ?? '';
  if (typeof href !== 'string' || (typeof title === 'string' && title !== '')) {
    return null;
  }
  const sameLink = (neighbour: JSONContent | undefined): boolean =>
    (neighbour?.marks ?? []).some((other) => other.type === 'link' && other.attrs?.['href'] === href);
  if (sameLink(previous) || sameLink(next)) {
    return null;
  }
  const uri = text === href && AUTOLINK_URI.test(href);
  const email = href === `mailto:${text}` && AUTOLINK_EMAIL.test(text);
  return uri || email ? { type: 'rawInline', attrs: { raw: `<${text}>`, kind: 'autolink' } } : null;
}

/** Hard breaks at the edges of a paragraph mean nothing in Markdown. */
function trimHardBreaks(content: JSONContent[]): JSONContent[] {
  let start = 0;
  let end = content.length;
  while (start < end && content[start]?.type === 'hardBreak') start += 1;
  while (end > start && content[end - 1]?.type === 'hardBreak') end -= 1;
  return start === 0 && end === content.length ? content : content.slice(start, end);
}

function prepareContent(input: JSONContent[], state: PrepareState): JSONContent[] {
  let previous: JSONContent | null = null;
  const content = input.some((child) => child.type === 'text') ? splitCodeEdges(input) : input;
  return content.map((child, index) => {
    let node: JSONContent = Array.isArray(child.content)
      ? { ...child, content: prepareContent(child.content, state) }
      : child;
    node = asAutolink(node, content[index - 1], content[index + 1]) ?? node;
    if (node.type === 'paragraph' && Array.isArray(node.content)) {
      node = { ...node, content: trimHardBreaks(node.content) };
    }
    if (node.type === 'text' && node.marks?.some((mark) => mark.type === 'code') === true) {
      // Each code span on its own: Markdown cannot change formatting inside
      // code, so the span closes where the other marks change (ADR 0014).
      state.codeRuns += 1;
      const attrs = { pwRun: state.codeRuns, pwText: (node.text ?? '').replace(/^\s+|\s+$/g, '') };
      node = {
        ...node,
        marks: node.marks.map((mark) => (mark.type === 'code' ? { type: 'code', attrs } : mark)),
      };
    }
    if (node.type === 'orderedList' && node.attrs?.['type'] !== undefined) {
      // Markdown numbers with digits only (the loss check names `a.`/`i.`).
      node = {
        ...node,
        attrs: Object.fromEntries(Object.entries(node.attrs).filter(([name]) => name !== 'type')),
      };
    }
    const kind = listKind(node.type);
    if (
      kind !== null &&
      previous !== null &&
      listKind(previous.type) === kind &&
      markerOf(previous, kind) === DEFAULT_MARKER[kind]
    ) {
      node = { ...node, attrs: { ...node.attrs, pwMarker: OTHER_MARKER[kind] } };
    }
    previous = node;
    return node;
  });
}

function markerOf(node: JSONContent, kind: ListKind): string {
  const marker: unknown = (node.attrs as Record<string, unknown> | undefined)?.['pwMarker'];
  return typeof marker === 'string' ? marker : DEFAULT_MARKER[kind];
}
