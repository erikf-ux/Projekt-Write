import { describe, expect, it } from 'vitest';
import { changeCounts, diffLines, MAX_EDITS, sideBySide, splitLines, type DiffLine } from './diff';

/** Applies an edit script: the old text from the same and removed lines, the new from same and added. */
function sides(lines: readonly DiffLine[]): [string[], string[]] {
  return [
    lines.filter((line) => line.kind !== 'added').map((line) => line.text),
    lines.filter((line) => line.kind !== 'removed').map((line) => line.text),
  ];
}

describe('diff', () => {
  it('splits lines without an empty last line', () => {
    expect(splitLines('')).toEqual([]);
    expect(splitLines('a\nb\n')).toEqual(['a', 'b']);
    expect(splitLines('a\r\nb')).toEqual(['a', 'b']);
    expect(splitLines('a\n\n')).toEqual(['a', '']);
  });

  it('finds the smallest set of changed lines', () => {
    const before = splitLines('Kapitel 1\nErster Satz.\nZweiter Satz.\nEnde\n');
    const after = splitLines('Kapitel 1\nErster Satz, länger.\nZweiter Satz.\nNeu\nEnde\n');
    const lines = diffLines(before, after);
    expect(lines).toEqual([
      { kind: 'same', text: 'Kapitel 1' },
      { kind: 'removed', text: 'Erster Satz.' },
      { kind: 'added', text: 'Erster Satz, länger.' },
      { kind: 'same', text: 'Zweiter Satz.' },
      { kind: 'added', text: 'Neu' },
      { kind: 'same', text: 'Ende' },
    ]);
    expect(changeCounts(lines)).toEqual({ removed: 1, added: 2 });
  });

  it('always reproduces both texts', () => {
    // Deterministic pseudo-random texts over a small alphabet of lines.
    let seed = 7;
    const next = () => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed;
    };
    for (let round = 0; round < 200; round += 1) {
      const a = Array.from({ length: next() % 30 }, () => `Zeile ${String(next() % 6)}`);
      const b = Array.from({ length: next() % 30 }, () => `Zeile ${String(next() % 6)}`);
      const lines = diffLines(a, b);
      expect(sides(lines)).toEqual([a, b]);
    }
  });

  it('puts removed and added lines of one change side by side', () => {
    const rows = sideBySide(diffLines(['a', 'b', 'c'], ['a', 'x', 'y', 'c']));
    expect(rows).toEqual([
      { left: { number: 1, text: 'a', kind: 'same' }, right: { number: 1, text: 'a', kind: 'same' } },
      { left: { number: 2, text: 'b', kind: 'removed' }, right: { number: 2, text: 'x', kind: 'added' } },
      { left: null, right: { number: 3, text: 'y', kind: 'added' } },
      { left: { number: 3, text: 'c', kind: 'same' }, right: { number: 4, text: 'c', kind: 'same' } },
    ]);
  });

  it('shows very different texts as one replaced block', () => {
    const a = Array.from({ length: MAX_EDITS }, (_, index) => `a${String(index)}`);
    const b = Array.from({ length: MAX_EDITS }, (_, index) => `b${String(index)}`);
    const lines = diffLines(a, b);
    expect(sides(lines)).toEqual([a, b]);
    expect(changeCounts(lines)).toEqual({ removed: MAX_EDITS, added: MAX_EDITS });
  });
});
