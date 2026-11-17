/**
 * The text the language check sees of a paragraph (section 14: "Code,
 * Inline-Code und URLs werden nicht geprüft").
 *
 * Offset `i` of the text is position `start + i` in the editor: text keeps
 * its UTF-16 units, inline code and links are masked with U+FFFC (one per
 * unit), a hard break is `\n`, other inline nodes are one U+FFFC. Code
 * blocks are not checked at all. A paragraph may hold parts in another
 * language (a `lang` mark): they become texts of their own.
 */
import type { Mark, Node as ProseMirrorNode } from '@tiptap/pm/model';

export const MASK = '￼';

/** A text to check and where it starts in the paragraph. */
export interface CheckPart {
  /** Offset in the paragraph's text. */
  offset: number;
  text: string;
  /** The marked language; null: the document's. */
  language: string | null;
}

export interface BlockText {
  /** The whole paragraph, masked. */
  text: string;
  parts: CheckPart[];
}

const cache = new WeakMap<ProseMirrorNode, BlockText | null>();

function masked(mark: Mark): boolean {
  return mark.type.name === 'code' || mark.type.name === 'link';
}

function languageOf(marks: readonly Mark[]): string | null {
  const mark = marks.find((m) => m.type.name === 'lang');
  const value: unknown = mark?.attrs['lang'];
  return typeof value === 'string' && value !== '' ? value : null;
}

/** Whether a block is checked at all. */
export function isChecked(block: ProseMirrorNode): boolean {
  return block.isTextblock && block.type.spec.code !== true && block.type.name !== 'rawBlock';
}

/**
 * The check text of a text block, null for blocks that are not checked.
 * Cached per node: unchanged blocks keep their node across transactions.
 */
export function blockText(block: ProseMirrorNode): BlockText | null {
  const known = cache.get(block);
  if (known !== undefined) {
    return known;
  }
  if (!isChecked(block)) {
    cache.set(block, null);
    return null;
  }
  let text = '';
  const parts: CheckPart[] = [];
  let current: CheckPart | null = null;
  block.forEach((child) => {
    let piece: string;
    if (child.isText) {
      const value = child.text ?? '';
      piece = child.marks.some(masked) ? MASK.repeat(value.length) : value;
    } else if (child.type.name === 'hardBreak') {
      piece = '\n';
    } else {
      piece = MASK.repeat(child.nodeSize);
    }
    const language = child.isText ? languageOf(child.marks) : (current?.language ?? null);
    if (!current || current.language !== language) {
      current = { offset: text.length, text: '', language };
      parts.push(current);
    }
    current.text += piece;
    text += piece;
  });
  const result = { text, parts: parts.filter((part) => part.text.trim() !== '') };
  cache.set(block, result);
  return result;
}

/** Letters, and hyphens or apostrophes between letters (as in Rust). */
const LETTER = /\p{L}|\p{M}/u;

/**
 * The word that ends right before `offset` in a check text: its start and
 * the word, or null when no letter precedes. Used when a word is finished.
 */
export function wordBefore(text: string, offset: number): { start: number; word: string } | null {
  let start = offset;
  while (start > 0) {
    const previous = text[start - 1] ?? '';
    if (LETTER.test(previous)) {
      start -= 1;
    } else if (
      /[-'’]/.test(previous) &&
      start - 2 >= 0 &&
      LETTER.test(text[start - 2] ?? '') &&
      start < offset
    ) {
      start -= 1;
    } else {
      break;
    }
  }
  // A surrogate pair: letters outside the BMP are two units.
  if (start > 0 && /[\uD800-\uDBFF]/.test(text[start - 1] ?? '')) {
    return null;
  }
  const word = text.slice(start, offset);
  return word === '' ? null : { start, word };
}
