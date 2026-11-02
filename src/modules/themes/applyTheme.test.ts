import { describe, expect, it } from 'vitest';
import { applyTheme } from './applyTheme';
import { themes, tokenVariables } from './tokens';

describe('applyTheme', () => {
  it('writes every token as a CSS custom property', () => {
    const root = document.createElement('div');
    applyTheme('light', root);
    for (const [token, variable] of Object.entries(tokenVariables)) {
      expect(root.style.getPropertyValue(variable)).toBe(themes.light[token as keyof typeof themes.light]);
    }
    expect(root.dataset['theme']).toBe('light');
    expect(root.style.colorScheme).toBe('light');
  });

  it('switches between dark and light', () => {
    const root = document.createElement('div');
    applyTheme('dark', root);
    expect(root.style.getPropertyValue('--pw-color-surface')).toBe('#1E1E1E');
    applyTheme('light', root);
    expect(root.style.getPropertyValue('--pw-color-surface')).toBe('#FFFFFF');
  });
});
