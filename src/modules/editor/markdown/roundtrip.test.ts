/**
 * Round-trip test of section 10: "Lege in `tests/fixtures/markdown/`
 * mindestens 50 Fälle an. Der Round-Trip-Test läuft von Import über Editor
 * zu Export. Inhalt und Struktur müssen gleich bleiben."
 *
 * Each fixture `NN-name.md` is read by the adapter, loaded into a Tiptap
 * editor with the Markdown schema, taken out of the editor again and saved.
 * The saved text must equal `NN-name.expected.md` if there is one, else the
 * input byte for byte. A fixture with an expected file names its
 * normalizations in `normalizations.json`, from a closed list (ADR 0014).
 *
 * Text equality is checked first; the document is also compared (not the
 * rendered HTML, which hid broken front matter and footnotes in the sample),
 * the probe must find nothing, and saving the saved text again changes
 * nothing.
 */
import { Editor, type JSONContent } from '@tiptap/core';
import { describe, expect, it } from 'vitest';
import manifest from '../../../../tests/fixtures/markdown/normalizations.json';
import { markdownExtensions } from '../schema/extensions';
import { parseMarkdown, serializeMarkdown } from './adapter';
import { firstDifference, probeMarkdown } from './probe';

const files = import.meta.glob<string>('/tests/fixtures/markdown/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

interface Fixture {
  name: string;
  input: string;
  expected: string | undefined;
}

const fixtures: Fixture[] = Object.keys(files)
  .filter((path) => !path.endsWith('.expected.md'))
  .sort()
  .map((path) => {
    const name = (path.split('/').pop() ?? path).replace(/\.md$/, '');
    return { name, input: files[path] ?? '', expected: files[path.replace(/\.md$/, '.expected.md')] };
  });

const knownNormalizations = new Set(Object.keys(manifest.normalizations));
const declared = manifest.fixtures as Record<string, string[] | undefined>;

/** Import → editor → export, as the app does it. */
function roundTrip(markdown: string): { markdown: string; body: JSONContent } {
  const document = parseMarkdown(markdown);
  const editor = new Editor({ extensions: markdownExtensions(), content: document.body });
  try {
    const body = editor.getJSON();
    return { markdown: serializeMarkdown({ frontMatter: document.frontMatter, body }), body };
  } finally {
    editor.destroy();
  }
}

describe('Markdown round trip (tests/fixtures/markdown)', () => {
  it('has at least 50 cases', () => {
    expect(fixtures.length).toBeGreaterThanOrEqual(50);
  });

  it('declares exactly the fixtures with an expected file, with known normalizations', () => {
    const withExpected = fixtures
      .filter((fixture) => fixture.expected !== undefined)
      .map((fixture) => fixture.name);
    expect(Object.keys(declared).sort()).toEqual(withExpected);
    for (const name of withExpected) {
      const normalizations = declared[name] ?? [];
      expect(normalizations.length, name).toBeGreaterThan(0);
      for (const normalization of normalizations) {
        expect(knownNormalizations.has(normalization), `${name}: ${normalization}`).toBe(true);
      }
    }
  });

  it.each(fixtures)('$name', ({ input, expected }) => {
    const result = roundTrip(input);
    expect(result.markdown).toBe(expected ?? input);
    // Reading the saved file gives the same document in the editor: an
    // expected file differs only by normalizations, not by content.
    const again = roundTrip(result.markdown);
    expect(firstDifference(result.body, again.body)).toBeNull();
    expect(parseMarkdown(result.markdown).frontMatter).toBe(parseMarkdown(input).frontMatter);
    expect(probeMarkdown(input).issues).toEqual([]);
    expect(again.markdown).toBe(result.markdown);
  });
});
