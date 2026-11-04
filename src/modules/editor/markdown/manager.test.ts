/**
 * The quick checks in front of marked's block rules (`manager.ts`) change
 * how fast a document is read, never what is read.
 */
import { Lexer } from 'marked';
import { describe, expect, it } from 'vitest';
import { lexBoth } from '@/test/plainMarked';
import { longGermanText } from '../../../../tests/e2e/longText';
import { THEMATIC_BREAK } from './manager';

const files = import.meta.glob<string>('/tests/fixtures/markdown/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

/** Edges of the five rules: thematic breaks, quotations, HTML, tables, setext headings. */
const EDGES = [
  '---\n',
  '  ***\n',
  '_ _ _\n',
  '- - -\n',
  '* \t* *\n',
  '\t---\n',
  '--\n',
  '-- -\n',
  '- -\n',
  '- a\n- b\n\n---\n',
  '- a\n---\n',
  '- item\n  ---\n',
  '* a\n* * *\n',
  'Titel\n===\n',
  'Titel\n---\n',
  'a\nb\n===\n\nc\n',
  'a\n  ===  \n',
  'a\n    ===\n',
  'Text\n\n===\n',
  'a\n\t===\n',
  '> Zitat\n',
  '   > Zitat\n> mehr\n',
  '    > Code\n',
  '<div>\nx\n</div>\n',
  '  <!-- Kommentar -->\n',
  '<u>unterstrichen</u> Text\n',
  'a < b\n',
  '| a | b |\n|---|---|\n| 1 | 2 |\n',
  'a | b\n- | -\n1 | 2\n',
  'a\n|-|\n',
  'a\n| - |\nb\n',
  'a\n:-:\n',
  '```\n---\n> x\n```\n',
  '1. eins\n2. zwei\n\n---\n',
  'Absatz\n- Liste\n',
  'Absatz\n> Zitat\n',
  'Absatz\n<div>\n',
];

describe('marked with quick checks', () => {
  it('has a thematic break rule with the same matches as marked', () => {
    const original = (Lexer as unknown as { rules: { block: { gfm: { hr: RegExp } } } }).rules.block.gfm.hr;
    const alphabet = ['-', '_', '*', ' ', '\t', '\n', 'a'];
    let seed = 1;
    const next = () => {
      seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
      return seed / 2_147_483_648;
    };
    for (let round = 0; round < 20_000; round += 1) {
      let text = '';
      for (let length = Math.floor(next() * 12); length > 0; length -= 1) {
        text += alphabet[Math.floor(next() * alphabet.length)] ?? '';
      }
      expect(THEMATIC_BREAK.exec(text)?.[0] ?? null, JSON.stringify(text)).toBe(
        original.exec(text)?.[0] ?? null,
      );
    }
  });

  it.each(EDGES)('reads %j like marked', (text) => {
    const { checked, plain } = lexBoth(text);
    expect(checked).toBe(plain);
  });

  it('reads every fixture and the long manuscript like marked', () => {
    const texts = [...Object.values(files), longGermanText(5_000).markdown];
    for (const text of texts) {
      const { checked, plain } = lexBoth(text);
      expect(checked).toBe(plain);
    }
  });
});
