import { beforeEach, describe, expect, it } from 'vitest';
import { applyTheme, installThemeStyles, themeStylesheet } from './applyTheme';
import { themes, tokenVariables, type ThemeName } from './tokens';

beforeEach(() => {
  document.getElementById('pw-theme-tokens')?.remove();
});

describe('themeStylesheet', () => {
  it('declares every token of every theme', () => {
    const css = themeStylesheet();
    for (const name of Object.keys(themes) as ThemeName[]) {
      const rule = css.split('\n').find((line) => line.startsWith(`[data-theme="${name}"]`));
      expect(rule).toBeDefined();
      for (const [token, variable] of Object.entries(tokenVariables)) {
        expect(rule).toContain(`${variable}: ${themes[name][token as keyof (typeof themes)[ThemeName]]};`);
      }
      expect(rule).toContain(`color-scheme: ${name};`);
    }
  });
});

describe('applyTheme', () => {
  it('switches themes through the data-theme attribute', () => {
    const root = document.createElement('div');
    document.body.append(root);
    applyTheme('light', root);
    expect(root.dataset['theme']).toBe('light');
    applyTheme('dark', root);
    expect(root.dataset['theme']).toBe('dark');
    // No colours are written inline; they come from the generated rules.
    expect(root.getAttribute('style')).toBeNull();
    root.remove();
  });

  it('keeps exactly one copy of the theme rules and replaces it on every switch', () => {
    installThemeStyles();
    const first = document.getElementById('pw-theme-tokens');
    applyTheme('dark');
    applyTheme('light');
    const styles = document.querySelectorAll('#pw-theme-tokens');
    expect(styles).toHaveLength(1);
    expect(styles[0]).not.toBe(first);
    expect(styles[0]?.textContent).toBe(themeStylesheet());
  });
});
