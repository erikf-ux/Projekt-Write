/**
 * What text has to look like in Markdown so that reading the file again gives
 * the same document (ADR 0014). A character is escaped only where it would
 * change the meaning: "Müller & Söhne" stays readable, `[[Wiki-Link]]` stays
 * a wiki link for other tools, `snake_case` keeps its underscores.
 */

const ASCII_PUNCTUATION = /[!-/:-@[-`{-~]/;
const LETTER_OR_DIGIT = /[\p{L}\p{N}]/u;
const ENTITY = /^&(?:#[0-9]{1,7}|#[xX][0-9A-Fa-f]{1,6}|[A-Za-z][A-Za-z0-9]{1,31});/;
const HTML_START = /[A-Za-z/!?]/;
const EMAIL_LOCAL = /[A-Za-z0-9._+-]/;
const EMAIL_DOMAIN = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*[A-Za-z0-9]/;
const SPECIAL = /[\\`*_~[\]&<:.@\u00a0]/g;

/**
 * Escapes text outside code. `inLink` is true for the text of a link, where
 * brackets are written as entities and addresses are not linked again.
 */
export function escapeText(text: string, inLink: boolean): string {
  return text.replace(SPECIAL, (character: string, offset: number) => {
    const before = text[offset - 1];
    const after = text[offset + 1];
    switch (character) {
      case '\\':
        // A backslash before a letter is a backslash already. Before a line
        // end it would be a hard break; before whitespace or at the end it
        // may end up next to the delimiter of a formatting.
        return after === undefined || /\s/.test(after) || ASCII_PUNCTUATION.test(after) ? '\\\\' : '\\';
      case '_':
        return before !== undefined &&
          after !== undefined &&
          LETTER_OR_DIGIT.test(before) &&
          LETTER_OR_DIGIT.test(after)
          ? '_'
          : '\\_';
      case '[':
        // In a link text marked removes the backslash of `\[` before reading
        // it again, so only an entity keeps it literal. `[^1]` would be a
        // footnote reference.
        if (inLink) return '&#91;';
        return after === '^' ? '\\[' : '[';
      case ']':
        // `](` would close a link, `]:` start a link definition.
        if (inLink) return '&#93;';
        return after === '(' || after === ':' ? '\\]' : ']';
      case '&':
        return ENTITY.test(text.slice(offset, offset + 40)) ? '&amp;' : '&';
      case '<':
        return after !== undefined && HTML_START.test(after) ? '&lt;' : '<';
      case ':':
        // `https://…` would become a link (GFM autolink).
        return !inLink &&
          text.startsWith('//', offset + 1) &&
          /(?:https?|ftp)$/i.test(text.slice(Math.max(0, offset - 5), offset))
          ? '\\:'
          : ':';
      case '.':
        return !inLink &&
          after !== undefined &&
          /[A-Za-z0-9-]/.test(after) &&
          /www$/i.test(text.slice(Math.max(0, offset - 3), offset))
          ? '\\.'
          : '.';
      case '@':
        return !inLink &&
          before !== undefined &&
          EMAIL_LOCAL.test(before) &&
          EMAIL_DOMAIN.test(text.slice(offset + 1, offset + 256))
          ? '\\@'
          : '@';
      case '\u00a0':
        // Visible in the source; a bare U+00A0 looks like a space.
        return '&nbsp;';
      default:
        // ` * ~ always start inline syntax.
        return `\\${character}`;
    }
  });
}

/** Escapes what would start a block at the beginning of a paragraph line. */
export function escapeLineStarts(markdown: string): string {
  return markdown.split('\n').map(escapeLineStart).join('\n');
}

const DELIMITER_ROW = /^ {0,3}\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;

function escapeLineStart(line: string): string {
  const escaped = line
    .replace(/^( {0,3})(#{1,6})(?=[ \t]|$)/, '$1\\$2')
    .replace(/^( {0,3})>/, '$1\\>')
    .replace(/^( {0,3})([-+])(?=[ \t]|$)/, '$1\\$2')
    .replace(/^( {0,3})(\d{1,9})([.)])(?=[ \t]|$)/, '$1$2\\$3')
    // A setext underline (`===`, `--`) or a thematic break (`--- ---`).
    .replace(/^( {0,3})(=+[ \t]*|-[- \t]*)$/, '$1\\$2')
    .replace(/^( {0,3})\[([ xX])\](?=[ \t]|$)/, '$1\\[$2]');
  // A table delimiter row (`--- | ---`) would turn the line above into a table.
  return DELIMITER_ROW.test(escaped) && /[|:]/.test(escaped) ? escaped.replace('-', '\\-') : escaped;
}

/** A link or image destination: bare if possible, otherwise in `<…>`. */
export function linkDestination(href: string): string {
  if (href !== '' && !/[\s<>()\\]/.test(href)) {
    return href;
  }
  return `<${href.replace(/[\\<>]/g, '\\$&').replace(/\n/g, '%0A')}>`;
}

/** A link or image title in double quotes, or nothing. */
export function linkTitle(title: string | null): string {
  return title === null || title === '' ? '' : ` "${title.replace(/["\\]/g, '\\$&')}"`;
}
