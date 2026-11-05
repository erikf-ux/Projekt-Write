/**
 * Line diff for comparing two versions side by side (section 13; ADR 0016,
 * decision 6). Myers' algorithm on lines after trimming the common start
 * and end. Very different texts (more than `MAX_EDITS` changed lines) are
 * shown as one replaced block instead of searching on.
 */

export type LineKind = 'same' | 'removed' | 'added';

export interface DiffLine {
  kind: LineKind;
  text: string;
}

/** One row of the side-by-side view; a side is empty where the other has a line. */
export interface SideBySideRow {
  left: { number: number; text: string; kind: 'same' | 'removed' } | null;
  right: { number: number; text: string; kind: 'same' | 'added' } | null;
}

export const MAX_EDITS = 4000;

export function splitLines(text: string): string[] {
  if (text === '') {
    return [];
  }
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  // A final line feed ends the last line; it does not start a new one.
  if (lines[lines.length - 1] === '') {
    lines.pop();
  }
  return lines;
}

/** The edit script turning `a` into `b`, line by line. */
export function diffLines(a: readonly string[], b: readonly string[]): DiffLine[] {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) {
    start += 1;
  }
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA -= 1;
    endB -= 1;
  }
  const head = a.slice(0, start).map((text): DiffLine => ({ kind: 'same', text }));
  const tail = a.slice(endA).map((text): DiffLine => ({ kind: 'same', text }));
  const middle = myers(a.slice(start, endA), b.slice(start, endB));
  return [...head, ...middle, ...tail];
}

function myers(a: readonly string[], b: readonly string[]): DiffLine[] {
  const n = a.length;
  const m = b.length;
  const replaced = (): DiffLine[] => [
    ...a.map((text): DiffLine => ({ kind: 'removed', text })),
    ...b.map((text): DiffLine => ({ kind: 'added', text })),
  ];
  if (n === 0 || m === 0) {
    return replaced();
  }
  const max = Math.min(n + m, MAX_EDITS);
  const offset = max;
  const v = new Int32Array(2 * max + 2);
  const trace: Int32Array[] = [];
  for (let d = 0; d <= max; d += 1) {
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      const down = k === -d || (k !== d && (v[offset + k - 1] ?? 0) < (v[offset + k + 1] ?? 0));
      let x = down ? (v[offset + k + 1] ?? 0) : (v[offset + k - 1] ?? 0) + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x += 1;
        y += 1;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) {
        return backtrack(trace, a, b, offset, d);
      }
    }
  }
  return replaced();
}

function backtrack(
  trace: readonly Int32Array[],
  a: readonly string[],
  b: readonly string[],
  offset: number,
  last: number,
): DiffLine[] {
  const lines: DiffLine[] = [];
  let x = a.length;
  let y = b.length;
  for (let d = last; d > 0; d -= 1) {
    const v = trace[d] as Int32Array;
    const k = x - y;
    const down = k === -d || (k !== d && (v[offset + k - 1] ?? 0) < (v[offset + k + 1] ?? 0));
    const previousK = down ? k + 1 : k - 1;
    const previousX = v[offset + previousK] ?? 0;
    const previousY = previousX - previousK;
    while (x > previousX && y > previousY) {
      x -= 1;
      y -= 1;
      lines.push({ kind: 'same', text: a[x] ?? '' });
    }
    if (down) {
      y -= 1;
      lines.push({ kind: 'added', text: b[y] ?? '' });
    } else {
      x -= 1;
      lines.push({ kind: 'removed', text: a[x] ?? '' });
    }
  }
  while (x > 0 && y > 0) {
    x -= 1;
    y -= 1;
    lines.push({ kind: 'same', text: a[x] ?? '' });
  }
  return lines.reverse();
}

/** Rows for two columns: removed and added lines of one change stand side by side. */
export function sideBySide(lines: readonly DiffLine[]): SideBySideRow[] {
  const rows: SideBySideRow[] = [];
  let left = 0;
  let right = 0;
  let index = 0;
  while (index < lines.length) {
    const line = lines[index] as DiffLine;
    if (line.kind === 'same') {
      left += 1;
      right += 1;
      rows.push({
        left: { number: left, text: line.text, kind: 'same' },
        right: { number: right, text: line.text, kind: 'same' },
      });
      index += 1;
      continue;
    }
    const removed: string[] = [];
    const added: string[] = [];
    while (index < lines.length && (lines[index] as DiffLine).kind !== 'same') {
      const change = lines[index] as DiffLine;
      (change.kind === 'removed' ? removed : added).push(change.text);
      index += 1;
    }
    for (let row = 0; row < Math.max(removed.length, added.length); row += 1) {
      const leftText = removed[row];
      const rightText = added[row];
      rows.push({
        left: leftText === undefined ? null : { number: (left += 1), text: leftText, kind: 'removed' },
        right: rightText === undefined ? null : { number: (right += 1), text: rightText, kind: 'added' },
      });
    }
  }
  return rows;
}

/** Counts of the changes, for the summary above the comparison. */
export function changeCounts(lines: readonly DiffLine[]): { removed: number; added: number } {
  let removed = 0;
  let added = 0;
  for (const line of lines) {
    if (line.kind === 'removed') removed += 1;
    else if (line.kind === 'added') added += 1;
  }
  return { removed, added };
}
