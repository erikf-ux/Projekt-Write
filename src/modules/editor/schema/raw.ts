/**
 * Raw Markdown that the editor keeps but never interprets (section 10:
 * "HTML-Blöcke und unbekanntes Inline-HTML … bleiben als nicht ausgeführter
 * Rohblock erhalten"; footnotes "Eigene Erweiterung oder Rohblock").
 *
 * * `rawBlock`: HTML blocks and footnote definitions, shown as source text.
 * * `rawInline`: inline HTML outside the allowed subset (`<u>`, `<br>`,
 *   `<img>` with `src`, `alt`, `title`, `width`, `height`) and footnote
 *   references (`[^1]`).
 *
 * Both are atoms: their text comes back unchanged when the document is saved.
 * They render as text (never as HTML), so nothing in them is ever executed.
 */
import { Extension, Node, type JSONContent } from '@tiptap/core';
import { stringAttribute } from './attributes';

/** Raw inline HTML as CommonMark defines it: tags, comments, declarations. */
const INLINE_HTML =
  /^(?:<\/?([A-Za-z][A-Za-z0-9-]*)(?:\s+[A-Za-z_:][\w.:-]*(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?)*\s*\/?>|<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<![A-Z]+[^>]*>|<!\[CDATA\[[\s\S]*?\]\]>)/;

const ATTRIBUTE = /([A-Za-z_:][\w.:-]*)(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?/g;
const IMAGE_ATTRIBUTES = new Set(['src', 'alt', 'title', 'width', 'height']);

/**
 * `<img>` of the allowed subset (section 10): `src` and otherwise only `alt`,
 * `title`, `width` and `height`, each at most once.
 */
export function isAllowedImageHtml(html: string): boolean {
  const match = /^<img((?:\s+[A-Za-z_:][\w.:-]*(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?)*)\s*\/?>$/i.exec(
    html.trim(),
  );
  if (!match) {
    return false;
  }
  const names = [...(match[1] ?? '').matchAll(ATTRIBUTE)].map((attribute) =>
    (attribute[1] ?? '').toLowerCase(),
  );
  return (
    names.includes('src') &&
    names.every((name) => IMAGE_ATTRIBUTES.has(name)) &&
    new Set(names).size === names.length
  );
}

/** True if `<u>` at the start of `src` has a closing `</u>` (as the underline tokenizer sees it). */
function opensUnderline(src: string): boolean {
  const delimiters = /\\[\s\S]|`+|<\/u>/gi;
  delimiters.lastIndex = 3;
  let match = delimiters.exec(src);
  while (match) {
    if (match[0].startsWith('`')) {
      const codeSpan = /^(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/.exec(src.slice(match.index));
      if (codeSpan) {
        delimiters.lastIndex = match.index + codeSpan[0].length;
      }
    } else if (!match[0].startsWith('\\')) {
      return match.index > 3;
    }
    match = delimiters.exec(src);
  }
  return false;
}

/**
 * Inline HTML the editor understands itself (section 10): a `<u>…</u>` pair,
 * `<br>` and `<img>` of the allowed subset. Everything else stays raw.
 */
function isAllowedInlineHtml(tag: string | undefined, html: string, src: string): boolean {
  switch (tag) {
    case 'u':
      return /^<u>$/i.test(html) && opensUnderline(src);
    case 'br':
      return /^<br\s*\/?>$/i.test(html);
    case 'img':
      return isAllowedImageHtml(html);
    default:
      return false;
  }
}

/** A footnote reference such as `[^1]` or `[^note]` (GFM); `[^x](url)` is a link. */
const FOOTNOTE_REFERENCE = /^\[\^[^\]\s]+\](?!\()/;

/**
 * A footnote definition: `[^id]: text`, continued by indented lines (also
 * after blank lines, as long as the next line is indented).
 */
const FOOTNOTE_DEFINITION = /^\[\^[^\]\s]+\]:[^\n]*(?:\n(?:[ \t]+[^\n]*|(?=\n*[ \t]+\S)))*/;

export const RawBlock = Node.create({
  name: 'rawBlock',
  group: 'block',
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      raw: { default: '' },
      kind: { default: 'html' },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-pw-raw-block]',
        getAttrs: (element) => ({
          raw: element.getAttribute('data-raw') ?? '',
          kind: element.getAttribute('data-kind') ?? 'html',
        }),
      },
    ];
  },

  renderHTML({ node }) {
    const raw = String(node.attrs['raw']);
    return [
      'div',
      {
        'data-pw-raw-block': '',
        'data-kind': String(node.attrs['kind']),
        'data-raw': raw,
        class: 'pw-raw-block',
      },
      ['code', {}, raw],
    ];
  },

  markdownTokenName: 'html',

  parseMarkdown: (token) => {
    if (token.type !== 'html' || token['block'] !== true) {
      return [];
    }
    const raw = (token.raw ?? '').replace(/\n+$/, '');
    // A lone <img> of the allowed subset becomes an image (section 10).
    if (isAllowedImageHtml(raw)) {
      return [];
    }
    return { type: 'rawBlock', attrs: { raw, kind: 'html' } };
  },

  renderMarkdown: (node: JSONContent) => stringAttribute(node, 'raw') ?? '',
});

export const RawInline = Node.create({
  name: 'rawInline',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      raw: { default: '' },
      kind: { default: 'html' },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-pw-raw-inline]',
        getAttrs: (element) => ({
          raw: element.getAttribute('data-raw') ?? '',
          kind: element.getAttribute('data-kind') ?? 'html',
        }),
      },
    ];
  },

  renderHTML({ node }) {
    const raw = String(node.attrs['raw']);
    return [
      'span',
      {
        'data-pw-raw-inline': '',
        'data-kind': String(node.attrs['kind']),
        'data-raw': raw,
        class: 'pw-raw-inline',
      },
      raw,
    ];
  },

  markdownTokenName: 'pwRawInline',

  markdownTokenizer: {
    name: 'pwRawInline',
    level: 'inline',
    start: (src: string) => {
      const index = src.search(/<[A-Za-z/!?]/);
      return index;
    },
    tokenize: (src: string) => {
      const match = INLINE_HTML.exec(src);
      if (!match) {
        return undefined;
      }
      if (isAllowedInlineHtml(match[1]?.toLowerCase(), match[0], src)) {
        return undefined;
      }
      return { type: 'pwRawInline', raw: match[0], text: match[0] };
    },
  },

  parseMarkdown: (token) => ({ type: 'rawInline', attrs: { raw: token.raw ?? '', kind: 'html' } }),

  renderMarkdown: (node: JSONContent) => stringAttribute(node, 'raw') ?? '',
});

/** Footnote references and definitions, kept as raw Markdown. */
export const Footnotes = Extension.create({
  name: 'footnotes',

  addExtensions() {
    return [FootnoteReference, FootnoteDefinition];
  },
});

const FootnoteReference = Extension.create({
  name: 'footnoteReference',
  markdownTokenName: 'pwFootnoteReference',
  markdownTokenizer: {
    name: 'pwFootnoteReference',
    level: 'inline',
    start: (src: string) => src.indexOf('[^'),
    tokenize: (src: string) => {
      const match = FOOTNOTE_REFERENCE.exec(src);
      return match ? { type: 'pwFootnoteReference', raw: match[0], text: match[0] } : undefined;
    },
  },
  parseMarkdown: (token) => ({ type: 'rawInline', attrs: { raw: token.raw ?? '', kind: 'footnote' } }),
});

const FootnoteDefinition = Extension.create({
  name: 'footnoteDefinition',
  markdownTokenName: 'pwFootnoteDefinition',
  markdownTokenizer: {
    name: 'pwFootnoteDefinition',
    level: 'block',
    start: (src: string) => src.search(/^\[\^[^\]\s]+\]:/m),
    tokenize: (src: string) => {
      const match = FOOTNOTE_DEFINITION.exec(src);
      return match ? { type: 'pwFootnoteDefinition', raw: match[0], text: match[0] } : undefined;
    },
  },
  parseMarkdown: (token) => ({
    type: 'rawBlock',
    attrs: { raw: (token.raw ?? '').replace(/\n+$/, ''), kind: 'footnote' },
  }),
});
