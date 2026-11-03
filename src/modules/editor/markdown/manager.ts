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
 */
import type { JSONContent, MarkdownToken } from '@tiptap/core';
import { MarkdownManager } from '@tiptap/markdown';
import { escapeText } from './escape';

/** Token types the fallback turns into content without losing anything. */
const PLAIN_TOKENS = new Set(['paragraph', 'heading', 'text', 'html', 'escape', 'space', 'def']);

/** Token types that hold nothing of their own (blank lines, link definitions). */
const EMPTY_TOKENS = new Set(['space', 'def']);

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

  constructor(options: ConstructorParameters<typeof MarkdownManager>[0]) {
    super(options);
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
