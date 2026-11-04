/**
 * Block tokenizers that stay linear in the length of the document (section
 * 6: "Dokument mit 50.000 Wörtern öffnen ≤ 500 ms").
 *
 * marked asks every block tokenizer at every block, and every `start`
 * before every paragraph, each time with the whole rest of the document.
 * A tokenizer that splits or searches that rest makes reading quadratic:
 * Tiptap's task list and table tokenizers split it into lines, a multiline
 * search runs to the end of the document. The functions here give the same
 * answers from the first lines only.
 */
import type { MarkdownTokenizer } from '@tiptap/core';

/**
 * Where a block could begin inside the paragraph at the start of `src`.
 * marked only cuts that paragraph with the answer, and a paragraph ends at
 * the first empty line, so a match after it changes nothing.
 */
export function startInParagraph(src: string, pattern: RegExp): number {
  const end = src.indexOf('\n\n');
  return (end < 0 ? src : src.slice(0, end)).search(pattern);
}

/** The delimiter row of a table (Tiptap's test). */
const TABLE_DELIMITER_ROW = /^[ \t|:]*-[ \t|:-]*$/;

/**
 * Tiptap's table `start`: 0 if the first line has a pipe and the second is
 * a delimiter row, else -1 – without splitting the rest into lines.
 */
export function tableStart(src: string): number {
  const first = src.indexOf('\n');
  if (first < 0) {
    return -1;
  }
  const end = src.indexOf('\n', first + 1);
  const delimiter = src.slice(first + 1, end < 0 ? src.length : end);
  if (!TABLE_DELIMITER_ROW.test(delimiter) || !delimiter.includes('|')) {
    return -1;
  }
  return src.slice(0, first).includes('|') ? 0 : -1;
}

/**
 * The start of a task list after blank lines, as Tiptap's tokenizer reads
 * it line by line (`/^(\s*)([-+*])\s+\[([ xX])\]\s+(.*)$/` per line).
 */
const TASK_LIST_START = /^(?:[^\S\n]*\n)*[^\S\n]*[-+*][^\S\n]+\[[ xX]\][^\S\n]+/;

/** Tiptap's task list tokenizer, asked only where a task list can start. */
export function guardedTaskListTokenizer(tokenizer: MarkdownTokenizer): MarkdownTokenizer {
  return {
    ...tokenizer,
    tokenize: (src, tokens, lexer) =>
      TASK_LIST_START.test(src) ? tokenizer.tokenize(src, tokens, lexer) : undefined,
  };
}

/** Tiptap's table tokenizer with a `start` that reads two lines only. */
export function fastTableTokenizer(tokenizer: MarkdownTokenizer): MarkdownTokenizer {
  return { ...tokenizer, start: tableStart };
}
