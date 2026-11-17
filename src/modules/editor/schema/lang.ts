/**
 * A part of a text in another language (section 14: "Manuelle
 * Sprachauswahl für … Textabschnitt", "Mehrsprachige Dokumente").
 *
 * - In the editor: `<span lang="en-GB" class="pw-lang">`; the language
 *   check takes the part as a text of its own in that language.
 * - In Markdown: exactly `<span lang="en-GB">…</span>`, so the file stays
 *   readable elsewhere and comes back byte for byte. Any other form of a
 *   span stays raw HTML as before.
 * - In `.pwrite` (format version 2): the mark `lang` with the attribute
 *   `lang`.
 */
import { Mark, mergeAttributes } from '@tiptap/core';

/** A language tag as Rust accepts it (`dictionaries.rs`, `is_tag`). */
export const LANGUAGE_TAG = /^[a-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/;
const OPEN = /^<span lang="([a-z]{2,3}(?:-[A-Za-z0-9]{2,8})*)">/;

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    languagePart: {
      /** Marks the selection as written in `lang`. */
      setLanguagePart: (lang: string) => ReturnType;
      /** Removes the language mark from the selection. */
      unsetLanguagePart: () => ReturnType;
    };
  }
}

/**
 * The end of a `<span lang="…">` part that starts `src`: the offset of its
 * `</span>`, or -1. Code spans and escapes are skipped; a nested span
 * keeps the whole thing raw.
 */
function closingSpan(src: string, from: number): number {
  const delimiters = /\\[\s\S]|`+|<\/span>|<span\b/gi;
  delimiters.lastIndex = from;
  let match = delimiters.exec(src);
  while (match) {
    const found = match[0].toLowerCase();
    if (found.startsWith('`')) {
      const codeSpan = /^(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/.exec(src.slice(match.index));
      if (codeSpan) {
        delimiters.lastIndex = match.index + codeSpan[0].length;
      }
    } else if (found === '</span>') {
      return match.index;
    } else if (found.startsWith('<span')) {
      return -1;
    }
    match = delimiters.exec(src);
  }
  return -1;
}

/** Whether `src` starts with a language part the tokenizer takes. */
export function opensLanguagePart(src: string): boolean {
  const open = OPEN.exec(src);
  return open !== null && closingSpan(src, open[0].length) > open[0].length;
}

export const LanguagePart = Mark.create({
  name: 'lang',

  addAttributes() {
    return {
      lang: {
        default: null,
        parseHTML: (element) => element.getAttribute('lang'),
        renderHTML: (attributes) =>
          typeof attributes['lang'] === 'string' ? { lang: attributes['lang'] } : {},
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[lang]',
        getAttrs: (element) => (LANGUAGE_TAG.test(element.getAttribute('lang') ?? '') ? null : false),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { class: 'pw-lang' }), 0];
  },

  parseMarkdown(token, helpers) {
    return helpers.applyMark('lang', helpers.parseInline(token.tokens ?? []), {
      lang: (token as { lang?: string }).lang,
    });
  },

  renderMarkdown(node, helpers) {
    const lang = String(node.attrs?.['lang'] ?? '');
    return `<span lang="${lang}">${helpers.renderChildren(node)}</span>`;
  },

  markdownTokenizer: {
    name: 'lang',
    level: 'inline',
    start(src) {
      return src.indexOf('<span lang="');
    },
    tokenize(src, _tokens, lexer) {
      const open = OPEN.exec(src);
      if (!open) {
        return undefined;
      }
      const end = closingSpan(src, open[0].length);
      if (end <= open[0].length) {
        return undefined;
      }
      const text = src.slice(open[0].length, end);
      return {
        type: 'lang',
        raw: src.slice(0, end + '</span>'.length),
        text,
        lang: open[1],
        tokens: lexer.inlineTokens(text),
      };
    },
  },

  addCommands() {
    return {
      setLanguagePart:
        (lang) =>
        ({ commands }) =>
          LANGUAGE_TAG.test(lang) ? commands.setMark(this.name, { lang }) : false,
      unsetLanguagePart:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
    };
  },
});
