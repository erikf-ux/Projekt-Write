/**
 * Writing statistics (section 12): words counted by Unicode rules,
 * characters, characters without spaces, paragraphs and reading time; for
 * the selection if there is one.
 *
 * * Words: word segments of UAX #29 (`Intl.Segmenter`, `isWordLike`), so
 *   "geht's" and "2,50" are one word, "E-Mail" and "z. B." are two.
 * * Characters: grapheme clusters, as people see them ("é", "👍🏽" are one).
 *   A hard break is one character; the boundary between two paragraphs or
 *   lines is none.
 * * Characters without spaces: graphemes that are not Unicode white space.
 * * Paragraphs: text blocks with something other than white space in rich
 *   text; groups of lines without an empty line between them in plain text.
 * * Raw HTML is not counted (it is markup); footnotes are, without their
 *   `[^1]:` marker.
 *
 * Counting goes block by block; a block keeps its counts until it changes,
 * so long documents stay fast while typing.
 */
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';

export interface TextStatistics {
  words: number;
  characters: number;
  charactersWithoutSpaces: number;
  paragraphs: number;
}

export const EMPTY_STATISTICS: TextStatistics = Object.freeze({
  words: 0,
  characters: 0,
  charactersWithoutSpaces: 0,
  paragraphs: 0,
});

let wordSegmenter: Intl.Segmenter | null = null;
let graphemeSegmenter: Intl.Segmenter | null = null;

const WHITE_SPACE = /^\p{White_Space}+$/u;

/** Words and characters of a piece of text (paragraphs are counted by the caller). */
export function countText(text: string): Omit<TextStatistics, 'paragraphs'> {
  if (text === '') {
    return { words: 0, characters: 0, charactersWithoutSpaces: 0 };
  }
  wordSegmenter ??= new Intl.Segmenter(undefined, { granularity: 'word' });
  graphemeSegmenter ??= new Intl.Segmenter(undefined, { granularity: 'grapheme' });
  let words = 0;
  for (const segment of wordSegmenter.segment(text)) {
    if (segment.isWordLike === true) {
      words += 1;
    }
  }
  let characters = 0;
  let withoutSpaces = 0;
  for (const { segment } of graphemeSegmenter.segment(text)) {
    characters += 1;
    if (!WHITE_SPACE.test(segment)) {
      withoutSpaces += 1;
    }
  }
  return { words, characters, charactersWithoutSpaces: withoutSpaces };
}

function add(a: TextStatistics, b: TextStatistics): TextStatistics {
  return {
    words: a.words + b.words,
    characters: a.characters + b.characters,
    charactersWithoutSpaces: a.charactersWithoutSpaces + b.charactersWithoutSpaces,
    paragraphs: a.paragraphs + b.paragraphs,
  };
}

/** Hard breaks are line breaks; images and raw HTML are not text. */
function leafText(leaf: ProseMirrorNode): string {
  return leaf.type.name === 'hardBreak' ? '\n' : '';
}

function asParagraph(text: string): TextStatistics {
  const counts = countText(text);
  return { ...counts, paragraphs: counts.charactersWithoutSpaces > 0 ? 1 : 0 };
}

/** The text of a footnote definition without its `[^id]:` marker. */
function footnoteText(raw: string): string {
  return raw.replace(/^\[\^[^\]\s]+\]:/, '');
}

/**
 * Statistics of a rich-text document. `cache` keeps the counts of nodes,
 * which stay the same objects until they change.
 */
export function richStatistics(
  document: ProseMirrorNode,
  cache: WeakMap<ProseMirrorNode, TextStatistics>,
): TextStatistics {
  const visit = (node: ProseMirrorNode): TextStatistics => {
    const cached = cache.get(node);
    if (cached) {
      return cached;
    }
    let result: TextStatistics = EMPTY_STATISTICS;
    if (node.isTextblock) {
      result = asParagraph(node.textBetween(0, node.content.size, undefined, leafText));
    } else if (node.type.name === 'rawBlock') {
      result =
        node.attrs['kind'] === 'footnote' ? asParagraph(footnoteText(String(node.attrs['raw']))) : result;
    } else {
      node.forEach((child) => {
        result = add(result, visit(child));
      });
    }
    cache.set(node, result);
    return result;
  };
  return visit(document);
}

/** Statistics of plain text and Markdown source, line by line. */
export function plainStatistics(
  document: ProseMirrorNode,
  cache: WeakMap<ProseMirrorNode, TextStatistics>,
): TextStatistics {
  let result = EMPTY_STATISTICS;
  let previousEmpty = true;
  document.forEach((line) => {
    let counts = cache.get(line);
    if (!counts) {
      counts = { ...countText(line.textContent), paragraphs: 0 };
      cache.set(line, counts);
    }
    const empty = counts.charactersWithoutSpaces === 0;
    // A paragraph starts at a non-empty line after an empty one.
    result = add(result, { ...counts, paragraphs: !empty && previousEmpty ? 1 : 0 });
    previousEmpty = empty;
  });
  return result;
}

/** Statistics of the selected text (`from`–`to` in the document). */
export function selectionStatistics(
  document: ProseMirrorNode,
  from: number,
  to: number,
  plain: boolean,
): TextStatistics {
  let result = EMPTY_STATISTICS;
  let previousEmpty = true;
  document.nodesBetween(from, to, (node, position) => {
    if (!node.isTextblock) {
      return true;
    }
    const start = Math.max(from, position + 1) - position - 1;
    const end = Math.min(to, position + 1 + node.content.size) - position - 1;
    const counts = countText(node.textBetween(start, end, undefined, leafText));
    const empty = counts.charactersWithoutSpaces === 0;
    const paragraphs = plain ? (!empty && previousEmpty ? 1 : 0) : empty ? 0 : 1;
    previousEmpty = empty;
    result = add(result, { ...counts, paragraphs });
    return false;
  });
  return result;
}

/** Reading time in whole minutes, at least 1 for any text. */
export function readingMinutes(words: number, wordsPerMinute: number): number {
  if (words === 0) {
    return 0;
  }
  return Math.max(1, Math.round(words / Math.max(1, wordsPerMinute)));
}
