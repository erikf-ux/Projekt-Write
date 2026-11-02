import { describe, expect, it } from 'vitest';
import { contrastRatio, MIN_GRAPHIC_CONTRAST, MIN_TEXT_CONTRAST } from './contrast';
import { themes, type ThemeName, type ThemeTokens } from './tokens';

type Pair = [foreground: keyof ThemeTokens, background: keyof ThemeTokens];

/** Text colours the start screen draws, and where it draws them. */
const textPairs: Pair[] = [
  ['text', 'surface'],
  ['textMuted', 'surface'],
  ['accent', 'surface'],
  ['error', 'surface'],
  ['success', 'surface'],
  ['text', 'sidebar'],
  ['textMuted', 'sidebar'],
  ['accent', 'sidebar'],
  ['error', 'sidebar'],
  ['success', 'sidebar'],
  // Primary button: surface-coloured label on accent background.
  ['surface', 'accent'],
];

/** Graphical elements: focus ring, status icons, button outline. */
const graphicPairs: Pair[] = [
  ['accent', 'surface'],
  ['accent', 'sidebar'],
  ['success', 'sidebar'],
  ['error', 'sidebar'],
];

/** Section 11: every cluster colour must reach 4.5:1 on the writing surface. */
const clusterPairs: Pair[] = [
  ['clusterBlue', 'surface'],
  ['clusterGreen', 'surface'],
  ['clusterViolet', 'surface'],
  ['clusterOrange', 'surface'],
  ['clusterGrey', 'surface'],
];

const themeNames = Object.keys(themes) as ThemeName[];

describe.each(themeNames)('%s theme', (name) => {
  const tokens = themes[name];

  it.each(textPairs)('text %s on %s reaches 4.5:1', (fg, bg) => {
    expect(contrastRatio(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
  });

  it.each(graphicPairs)('graphic %s on %s reaches 3:1', (fg, bg) => {
    expect(contrastRatio(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(MIN_GRAPHIC_CONTRAST);
  });

  it.each(clusterPairs)('cluster %s on %s reaches 4.5:1', (fg, bg) => {
    expect(contrastRatio(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
  });
});

describe('contrastRatio', () => {
  it('matches known reference values', () => {
    expect(contrastRatio('#000000', '#FFFFFF')).toBeCloseTo(21, 5);
    expect(contrastRatio('#FFFFFF', '#FFFFFF')).toBeCloseTo(1, 5);
    expect(contrastRatio('#767676', '#FFFFFF')).toBeCloseTo(4.54, 2);
  });

  it('rejects malformed colours', () => {
    expect(() => contrastRatio('red', '#FFFFFF')).toThrow();
  });

  it('documents the one combination below 4.5:1: light orange on the side bar', () => {
    // Allowed only as a graphical element (>= 3:1), never as text.
    const ratio = contrastRatio(themes.light.clusterOrange, themes.light.sidebar);
    expect(ratio).toBeLessThan(MIN_TEXT_CONTRAST);
    expect(ratio).toBeGreaterThanOrEqual(MIN_GRAPHIC_CONTRAST);
  });
});
