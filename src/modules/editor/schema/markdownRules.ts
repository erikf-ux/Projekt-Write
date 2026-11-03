/**
 * Markdown rules of Project Write on top of `@tiptap/markdown` (ADR 0014).
 * Each rule keeps a document the same when it is saved and read again.
 *
 * * Document: exactly one blank line between blocks.
 * * Paragraph: no whitespace at the edges; a line that would start a block
 *   (`# `, `> `, `- `, `1. `, a setext underline) is escaped; an empty
 *   paragraph is written as `&nbsp;` and read back as an empty paragraph.
 * * Heading: a trailing `#` is escaped, a line break is written as `<br>`.
 * * Inline code may carry other formatting (`**\`x\`**`, a link around
 *   code), as in Markdown; Tiptap's code excludes all other marks.
 * * Code block: the fence is longer than any backtick run in the code.
 * * Link: destination in `<…>` where needed; a link whose text is its
 *   address is written as `<address>` by the adapter.
 * * List item: every line after the marker is indented to the content
 *   column; paragraphs, thematic breaks, HTML and tables get a blank line.
 * * Lists: adjacent lists get different markers (`-`/`*`, `.`/`)`), or
 *   Markdown would read them as one list. The adapter sets `pwMarker`.
 *   Ordered lists are read as CommonMark defines them (digits only).
 * * Linked image `[![alt](src)](href)`: kept as an image with link target
 *   (section 10: "Bild in einem Link … Eigene Regel").
 * * HTML entities (`&copy;`) become characters, exactly once; code keeps
 *   them literally.
 */
import {
  Extension,
  type JSONContent,
  type MarkdownParseHelpers,
  type MarkdownRendererHelpers,
  type MarkdownToken,
} from '@tiptap/core';
import Code from '@tiptap/extension-code';
import CodeBlock from '@tiptap/extension-code-block';
import Document from '@tiptap/extension-document';
import HardBreak from '@tiptap/extension-hard-break';
import Heading from '@tiptap/extension-heading';
import Link from '@tiptap/extension-link';
import { BulletList, ListItem, OrderedList, TaskItem, TaskList } from '@tiptap/extension-list';
import Paragraph from '@tiptap/extension-paragraph';
import { escapeLineStarts, linkDestination, linkTitle } from '../markdown/escape';
import { tokenString } from './attributes';

/** Written for an empty paragraph; read back as an empty paragraph. */
export const EMPTY_PARAGRAPH = '&nbsp;';

function contentOf(node: JSONContent): JSONContent[] {
  return Array.isArray(node.content) ? node.content : [];
}

function attribute(node: JSONContent, name: string): unknown {
  return (node.attrs as Record<string, unknown> | undefined)?.[name];
}

export const MarkdownDocument = Document.extend({
  renderMarkdown(node, helpers) {
    return contentOf(node)
      .map((child, index) =>
        helpers.renderChild ? helpers.renderChild(child, index) : helpers.renderChildren([child]),
      )
      .map((block) => block.replace(/^\n+|\n+$/g, ''))
      .filter((block) => block !== '')
      .join('\n\n');
  },
});

/** A footnote reference followed by `(` would read as a link (`[^1](…)`). */
function escapeAfterFootnotes(markdown: string): string {
  return markdown.replace(/(\[\^[^\]\s]+\])\(/g, '$1\\(');
}

export const MarkdownParagraph = Paragraph.extend({
  parseMarkdown(token, helpers) {
    const content = helpers.parseInline(token.tokens ?? []);
    const [only] = content;
    const empty = content.length === 1 && only?.type === 'text' && only.text === ' ';
    return helpers.createNode('paragraph', undefined, empty ? [] : content);
  },

  renderMarkdown(node, helpers, context) {
    // Table cells hold inline content only: no block can start there, and a
    // line break of the text is a space (a hard break, "  \n", stays).
    const inTable = context.parentType === 'table';
    const rendered = helpers.renderChildren(contentOf(node)).replace(/^[ \t\n]+|[ \t\n]+$/g, '');
    if (rendered === '') {
      return inTable ? '' : EMPTY_PARAGRAPH;
    }
    // Whitespace around a line break: a hard break keeps exactly two spaces
    // and absorbs a line break next to it, the next line starts without
    // whitespace (CommonMark ignores it there), and no blank line appears.
    const breaks = rendered.replace(/[ \t]*\n[ \t\n]*/g, (run) => (/ {2}\n/.test(run) ? '  \n' : '\n'));
    const footnotes = escapeAfterFootnotes(breaks);
    return inTable ? footnotes.replace(/(?<! {2})\n/g, ' ') : escapeLineStarts(footnotes);
  },
});

export const MarkdownHeading = Heading.extend({
  renderMarkdown(node, helpers) {
    const level = Math.min(6, Math.max(1, Number(attribute(node, 'level')) || 1));
    const marker = '#'.repeat(level);
    // A setext heading may span lines; an ATX heading is one line.
    const text = helpers
      .renderChildren(contentOf(node))
      .replace(/[ \t]*\n[ \t]*/g, ' ')
      .replace(/^[ \t]+|[ \t]+$/g, '');
    if (text === '') {
      return marker;
    }
    // A trailing `#` run would be read as the closing sequence.
    return `${marker} ${escapeAfterFootnotes(text).replace(/(^|[ \t])(#+)$/, '$1\\$2')}`;
  },
});

export const MarkdownHardBreak = HardBreak.extend({
  renderMarkdown(_node, _helpers, context) {
    // A heading ends at the line end, and a line of only two spaces would
    // be a blank line; `<br>` is part of the allowed subset.
    return context.parentType === 'heading' || context.previousNode?.type === 'hardBreak' ? '<br>' : '  \n';
  },
});

/** The shortest backtick fence that does not occur in the code. */
function codeFence(code: string): string {
  const runs = new Set((code.match(/`+/g) ?? []).map((run) => run.length));
  let length = 1;
  while (runs.has(length)) length += 1;
  return '`'.repeat(length);
}

export const MarkdownCode = Code.extend({
  excludes: '',

  renderMarkdown(node, helpers, context) {
    // The adapter puts the text of this one span into the mark (`pwText`);
    // meta.markText joins all adjacent code spans.
    const own = attribute(node, 'pwText');
    const markText: unknown = context.meta?.['markText'];
    const text = typeof own === 'string' ? own : typeof markText === 'string' ? markText : '';
    // Tiptap writes whitespace at the edges outside the mark.
    const code = text.replace(/\n/g, ' ').replace(/^\s+|\s+$/g, '');
    const fence = codeFence(code);
    const padding = code.startsWith('`') || code.endsWith('`') ? ' ' : '';
    return `${fence}${padding}${helpers.renderChildren(contentOf(node))}${padding}${fence}`;
  },
});

function longestRun(text: string, character: '`' | '~'): number {
  let longest = 0;
  let current = 0;
  for (const value of text) {
    current = value === character ? current + 1 : 0;
    longest = Math.max(longest, current);
  }
  return longest;
}

export const MarkdownCodeBlock = CodeBlock.extend({
  renderMarkdown(node) {
    const language = attribute(node, 'language');
    const info = typeof language === 'string' ? language : '';
    const code = contentOf(node)
      .map((child) => child.text ?? '')
      .join('');
    // A backtick fence cannot have backticks in its info string.
    const character = info.includes('`') ? '~' : '`';
    const fence = character.repeat(Math.max(3, longestRun(code, character) + 1));
    return code === '' ? `${fence}${info}\n${fence}` : `${fence}${info}\n${code}\n${fence}`;
  },
});

export const MarkdownLink = Link.extend({
  renderMarkdown(node, helpers, context) {
    const href = attribute(node, 'href');
    const title = attribute(node, 'title');
    const destination = typeof href === 'string' ? href : '';
    const titleText = typeof title === 'string' && title !== '' ? title : null;
    const text = helpers.renderChildren(contentOf(node));
    // Marks are rendered around a placeholder; meta.markText is the link text.
    // Autolinks (`<https://…>`) are written by the adapter as raw text.
    const markText: unknown = context.meta?.['markText'];
    // `[^x](…)` would start with a footnote reference.
    const caret = typeof markText === 'string' && markText.startsWith('^') ? '\\' : '';
    return `[${caret}${text}](${linkDestination(destination)}${linkTitle(titleText)})`;
  },
});

function isEmptyParagraphNode(node: JSONContent): boolean {
  return node.type === 'paragraph' && contentOf(node).length === 0;
}

/**
 * Blocks that need a blank line before them inside a list item: a paragraph
 * would continue the one above, a thematic break would underline it (setext
 * heading), an HTML block of type 7 could not interrupt it.
 */
const BLANK_LINE_BEFORE = new Set(['paragraph', 'horizontalRule', 'rawBlock', 'table']);

/**
 * A list item: the marker, the first block after it, every further line
 * indented to the content column (also continuation lines of the first
 * paragraph), blank lines without trailing spaces.
 */
function renderListItem(
  node: JSONContent,
  helpers: MarkdownRendererHelpers,
  marker: string,
  column: number,
  task = false,
): string {
  const indent = ' '.repeat(column);
  const children = contentOf(node);
  // An item that starts with a block (`- # Titel`, `- ```js`): the schema
  // holds an empty paragraph before it, Markdown writes the block after the
  // marker. Not for a paragraph (it would fill the empty one), a thematic
  // break (`- ---` is a thematic break itself) or a task (`- [ ]` needs text).
  const [first, second] = children;
  const blockFirst =
    !task &&
    first !== undefined &&
    isEmptyParagraphNode(first) &&
    second !== undefined &&
    second.type !== 'paragraph' &&
    second.type !== 'horizontalRule';
  let output = '';
  children.forEach((child, index) => {
    if (blockFirst && index === 0) {
      return;
    }
    const rendered = (
      helpers.renderChild ? helpers.renderChild(child, index) : helpers.renderChildren([child])
    ).replace(/^\n+|\n+$/g, '');
    const starts = output === '';
    const lines = rendered
      .split('\n')
      .map((line, number) => ((starts && number === 0) || line === '' ? line : indent + line));
    if (starts) {
      output = marker + lines.join('\n');
    } else {
      output += (BLANK_LINE_BEFORE.has(child.type ?? '') ? '\n\n' : '\n') + lines.join('\n');
    }
  });
  return output === '' ? marker.trimEnd() : output;
}

/**
 * The blocks of a list item as CommonMark reads them. Tiptap's own parser
 * turns a heading at the start into a paragraph, `- 1. x` into text and
 * keeps text after an HTML block as bare text.
 */
function listItemBlocks(tokens: MarkdownToken[], helpers: MarkdownParseHelpers): JSONContent[] {
  const blocks = tokens.flatMap((token): JSONContent[] => {
    if (token.type === 'space') {
      return [];
    }
    if (token.type === 'text') {
      const inline = token.tokens ?? helpers.tokenizeInline?.(token.text ?? '') ?? [];
      return [helpers.createNode('paragraph', undefined, helpers.parseInline(inline))];
    }
    return helpers.parseChildren([token]);
  });
  // The schema wants a paragraph first (`paragraph block*`).
  return blocks[0]?.type === 'paragraph' ? blocks : [{ type: 'paragraph', content: [] }, ...blocks];
}

export const MarkdownListItem = ListItem.extend({
  parseMarkdown(token, helpers) {
    if (token.type !== 'list_item') {
      return [];
    }
    return helpers.createNode('listItem', undefined, listItemBlocks(token.tokens ?? [], helpers));
  },

  renderMarkdown(node, helpers, context) {
    if (context.parentType === 'orderedList') {
      const start = Number(
        (context.meta?.['parentAttrs'] as Record<string, unknown> | undefined)?.['start'] ?? 1,
      );
      const marker = `${String((Number.isFinite(start) ? start : 1) + context.index)}. `;
      return renderListItem(node, helpers, marker, marker.length);
    }
    return renderListItem(node, helpers, '- ', 2);
  },
});

export const MarkdownTaskItem = TaskItem.extend({
  // Nested blocks of a task item come as marked's `text` tokens when the
  // task list is itself inside a list; Tiptap would keep them as bare text.
  parseMarkdown(token, helpers) {
    const nested = token['nestedTokens'] as MarkdownToken[] | undefined;
    const blocks = nested?.map((child) =>
      child.type === 'text'
        ? {
            ...child,
            type: 'paragraph',
            tokens: child.tokens ?? helpers.tokenizeInline?.(child.text ?? '') ?? [],
          }
        : child,
    );
    return this.parent?.({ ...token, nestedTokens: blocks }, helpers) ?? [];
  },

  renderMarkdown(node, helpers) {
    // The content column of `- [ ] text` is that of the bullet `- `.
    return renderListItem(node, helpers, attribute(node, 'checked') === true ? '- [x] ' : '- [ ] ', 2, true);
  },
});

/** The marker the adapter chose for a list next to a list of the same kind. */
function withBulletMarker(rendered: string, node: JSONContent): string {
  return attribute(node, 'pwMarker') === '*' ? rendered.replace(/^-(?= |$)/gm, '*') : rendered;
}

export const MarkdownBulletList = BulletList.extend({
  renderMarkdown(node, helpers, context) {
    return withBulletMarker(this.parent?.(node, helpers, context) ?? '', node);
  },
});

export const MarkdownTaskList = TaskList.extend({
  renderMarkdown(node, helpers, context) {
    return withBulletMarker(this.parent?.(node, helpers, context) ?? '', node);
  },
});

export const MarkdownOrderedList = OrderedList.extend({
  // Tiptap's own tokenizer also reads `a.`, `iv.` and `Dr.` as list markers,
  // so "Dr. Müller kommt." would become a list. marked follows CommonMark.
  markdownTokenizer: {
    name: 'orderedList',
    level: 'block',
    start: () => -1,
    tokenize: () => undefined,
  },

  // The items of marked's own list token, read like those of a bullet list.
  parseMarkdown: (token, helpers) => {
    if (token.type !== 'list' || token['ordered'] !== true) {
      return [];
    }
    const start: unknown = token['start'];
    const content = helpers.parseChildren(token.items ?? []);
    return typeof start === 'number' && start !== 1
      ? { type: 'orderedList', attrs: { start }, content }
      : { type: 'orderedList', content };
  },

  renderMarkdown(node, helpers, context) {
    const rendered = this.parent?.(node, helpers, context) ?? '';
    return attribute(node, 'pwMarker') === ')' ? rendered.replace(/^(\d{1,9})\.(?= |$)/gm, '$1)') : rendered;
  },
});

/** `[![alt](src "title")](href "title")` as one token. */
const LINKED_IMAGE =
  /^\[!\[((?:[^\]\\]|\\.)*)\]\(\s*(<[^>]*>|[^\s)]+)(?:\s+"((?:[^"\\]|\\.)*)")?\s*\)\]\(\s*(<[^>]*>|[^\s)]+)(?:\s+"((?:[^"\\]|\\.)*)")?\s*\)/;

function unescape(text: string): string {
  return text.replace(/\\([!-/:-@[-`{-~])/g, '$1');
}

function destination(text: string): string {
  return text.startsWith('<') && text.endsWith('>') ? unescape(text.slice(1, -1)) : unescape(text);
}

export const LinkedImages = Extension.create({
  name: 'linkedImages',
  markdownTokenName: 'pwLinkedImage',
  markdownTokenizer: {
    name: 'pwLinkedImage',
    level: 'inline',
    start: (src: string) => src.indexOf('[!['),
    tokenize: (src: string) => {
      const match = LINKED_IMAGE.exec(src);
      if (!match) {
        return undefined;
      }
      return {
        type: 'pwLinkedImage',
        raw: match[0],
        text: unescape(match[1] ?? ''),
        src: destination(match[2] ?? ''),
        title: match[3] === undefined ? null : unescape(match[3]),
        href: destination(match[4] ?? ''),
        linkTitle: match[5] === undefined ? null : unescape(match[5]),
      };
    },
  },
  parseMarkdown: (token) => ({
    type: 'image',
    attrs: {
      src: tokenString(token['src']) ?? '',
      alt: token.text ?? '',
      title: tokenString(token['title']),
      href: tokenString(token['href']) ?? '',
      linkTitle: tokenString(token['linkTitle']),
    },
  }),
});

const ENTITY = /^&(?:[A-Za-z][A-Za-z0-9]{1,31}|#[0-9]{1,7}|#[xX][0-9A-Fa-f]{1,6});/;

let decoder: HTMLTextAreaElement | null = null;

/** The character of an HTML entity, or the entity itself if unknown. */
export function decodeEntity(entity: string): string {
  decoder ??= document.createElement('textarea');
  decoder.innerHTML = entity;
  return decoder.value;
}

export const Entities = Extension.create({
  name: 'entities',
  markdownTokenName: 'pwEntity',
  markdownTokenizer: {
    name: 'pwEntity',
    level: 'inline',
    start: (src: string) => src.indexOf('&'),
    tokenize: (src: string) => {
      const match = ENTITY.exec(src);
      return match ? { type: 'pwEntity', raw: match[0], text: match[0] } : undefined;
    },
  },
  parseMarkdown: (token) => ({ type: 'text', text: decodeEntity(token.raw ?? '') }),
});
