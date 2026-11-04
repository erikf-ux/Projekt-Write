/**
 * The Markdown manager of `@tiptap/markdown` with readable text encoding.
 *
 * The original escapes every `&`, `<` and `>` as an HTML entity, so
 * "Müller & Söhne" would be saved as "Müller &amp; Söhne", and every `_` and
 * `[`, also inside words and wiki links. Here a character is escaped only
 * where it would change the meaning when the file is read again (ADR 0014).
 *
 * Three internal methods are replaced (pinned to Tiptap 3.32.2, covered by
 * the round-trip tests):
 *
 * * `encodeTextForMarkdown`: the readable escaping of `escape.ts`.
 * * `parseTokens`: blank lines never become empty paragraphs.
 * * `parseFallbackToken`: a token no extension handles is recorded in
 *   `unsupported`, so the round-trip probe can name it.
 *
 * The manager has its own marked instance with quick checks in front of
 * five block rules (`blockRules` below).
 */
import type { JSONContent, MarkdownToken } from '@tiptap/core';
import { MarkdownManager } from '@tiptap/markdown';
import { Marked, type TokenizerObject } from 'marked';
import { escapeText } from './escape';

/** The first lines of `src` up to its first empty line (a paragraph ends there). */
function firstParagraph(src: string): string {
  const end = src.indexOf('\n\n');
  return end < 0 ? src : src.slice(0, end + 1);
}

/**
 * marked tries every block rule at every block, each time on the whole rest
 * of the document. JavaScriptCore (WebKit, the webview on Linux) cannot
 * compile some of these expressions, such as `(?:-[\t ]*){3,}` of the
 * thematic break, and its interpreter tries every start position despite
 * the `^`: each failed attempt costs the length of the rest. Reading 50,000
 * words took 7 s instead of 30 ms (section 6: "Dokument mit 50.000 Wörtern
 * öffnen ≤ 500 ms").
 *
 * Each of these rules can only match where its first or second line, or a
 * line of its paragraph, has a certain form. Where it has not, the check
 * answers "no token" (`undefined`) at once; otherwise `false` lets marked
 * run its own rule, so every token stays exactly as marked reads it.
 */
const blockRules: TokenizerObject = {
  // ` {0,3}>`.
  blockquote: (src) => (/^ {0,3}>/.test(src) ? false : undefined),
  // Every kind of HTML block starts with ` {0,3}<`.
  html: (src) => (/^ {0,3}</.test(src) ? false : undefined),
  // The second line is the delimiter row: `|`, `:`, `-` and spaces, with a `-`.
  table: (src) => {
    const first = src.indexOf('\n');
    return first >= 0 && /^[ |:-]*-[ |:-]*(?:\n|$)/.test(src.slice(first + 1)) ? false : undefined;
  },
  // A setext underline (`===`, `---`) before the paragraph ends.
  lheading: (src) => (/\n {0,3}(?:=+|-+) *(?:\n|$)/.test(firstParagraph(src)) ? false : undefined),
};

/**
 * marked's thematic break (`hr`), unrolled: `(?:-[\t ]*){3,}` is three
 * dashes with spaces or tabs after each, then any of them. Same matches
 * (`manager.test.ts`), but JavaScriptCore compiles it. The list tokenizer
 * tests it against the rest of the document at every list item.
 */
export const THEMATIC_BREAK =
  /^ {0,3}(-[\t ]*-[\t ]*-[-\t ]*|_[ \t]*_[ \t]*_[_ \t]*|\*[ \t]*\*[ \t]*\*[*\t ]*)(?:\n+|$)/;

interface TokenizerRules {
  block: Record<string, RegExp>;
}

/** A marked instance of its own, with the checks of `blockRules` and `THEMATIC_BREAK`. */
function createMarked(): Marked {
  const instance = new Marked();
  instance.use({ tokenizer: blockRules });
  const tokenizer = instance.defaults.tokenizer;
  if (tokenizer) {
    // Every lexer hands the tokenizer marked's shared rules; it keeps a copy
    // with the thematic break above, so marked's own objects stay unchanged.
    let rules: TokenizerRules | undefined;
    Object.defineProperty(tokenizer, 'rules', {
      configurable: true,
      get: () => rules,
      set: (value: TokenizerRules) => {
        rules = { ...value, block: { ...value.block, hr: THEMATIC_BREAK } };
      },
    });
  }
  return instance;
}

/** Token types the fallback turns into content without losing anything. */
const PLAIN_TOKENS = new Set(['paragraph', 'heading', 'text', 'html', 'escape', 'space', 'def']);

/** Token types that hold nothing of their own (blank lines, link definitions). */
const EMPTY_TOKENS = new Set(['space', 'def']);

type MarkdownMarked = NonNullable<NonNullable<ConstructorParameters<typeof MarkdownManager>[0]>['marked']>;

interface Internals {
  parseTokens: (tokens: MarkdownToken[], implicitEmptyParagraphs?: boolean) => JSONContent[];
  parseFallbackToken: (token: MarkdownToken, implicitEmptyParagraphs?: boolean) => unknown;
  encodeTextForMarkdown: (text: string, node: JSONContent, parent?: JSONContent | null) => string;
}

function encodeText(text: string, node: JSONContent, parent?: JSONContent | null): string {
  if (parent?.type === 'codeBlock') {
    return text;
  }
  if ((node.marks ?? []).some((mark) => mark.type === 'code')) {
    // A code span has no line breaks (CommonMark reads them as spaces).
    return text.replace(/\n/g, ' ');
  }
  const inLink = (node.marks ?? []).some((mark) => mark.type === 'link');
  // Spaces before a line break would turn it into a hard break.
  return escapeText(text.replace(/[ \t]+\n/g, '\n'), inLink);
}

export class ProjectWriteMarkdownManager extends MarkdownManager {
  /** Token types of the last `parse` that no extension could hold. */
  readonly unsupported = new Set<string>();

  constructor(options: Omit<NonNullable<ConstructorParameters<typeof MarkdownManager>[0]>, 'marked'>) {
    // Tiptap only uses what an instance has: `Lexer`, `lexer`, `use`, `defaults`.
    super({ ...options, marked: createMarked() as unknown as MarkdownMarked });
    const internals = this as unknown as Internals;
    const parseTokens = internals.parseTokens.bind(this);
    const parseFallbackToken = internals.parseFallbackToken.bind(this);
    internals.parseTokens = (tokens) => parseTokens(tokens, false);
    internals.parseFallbackToken = (token, implicitEmptyParagraphs) => {
      const result = parseFallbackToken(token, implicitEmptyParagraphs);
      const type = token.type ?? '';
      if (!PLAIN_TOKENS.has(type) || (result === null && !EMPTY_TOKENS.has(type))) {
        this.unsupported.add(type);
      }
      return result;
    };
    internals.encodeTextForMarkdown = encodeText;
  }

  override parse(markdown: string): JSONContent {
    this.unsupported.clear();
    return super.parse(markdown);
  }
}
