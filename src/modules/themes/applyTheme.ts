import { themes, tokenVariables, type ThemeName, type ThemeTokens } from './tokens';

const DARK_QUERY = '(prefers-color-scheme: dark)';
const STYLE_ID = 'pw-theme-tokens';

/**
 * CSS rules with the custom properties of every theme, generated from
 * tokens.ts (the only source of colours):
 * `[data-theme="dark"] { --pw-color-surface: …; color-scheme: dark }`.
 *
 * Switching a theme sets the `data-theme` attribute and replaces this
 * stylesheet. WebKit on macOS kept stale inherited colours for some elements
 * when only custom properties changed (inline or via the attribute); a
 * stylesheet change makes every engine recompute all styles. Unlike the
 * `display: none` trick it keeps focus and scroll position.
 */
export function themeStylesheet(): string {
  const tokenNames = Object.keys(tokenVariables) as (keyof ThemeTokens)[];
  return (Object.keys(themes) as ThemeName[])
    .map((name) => {
      const declarations = tokenNames.map((token) => `${tokenVariables[token]}: ${themes[name][token]};`);
      return `[data-theme="${name}"] { ${declarations.join(' ')} color-scheme: ${name}; }`;
    })
    .join('\n');
}

/** Adds the theme rules to the document, replacing an existing copy. */
export function installThemeStyles(doc: Document = document): void {
  const style = doc.createElement('style');
  style.id = STYLE_ID;
  style.textContent = themeStylesheet();
  const existing = doc.getElementById(STYLE_ID);
  if (existing) {
    existing.replaceWith(style);
  } else {
    doc.head.append(style);
  }
}

/** Activates `theme` for `root` and everything inside it. */
export function applyTheme(theme: ThemeName, root: HTMLElement = document.documentElement): void {
  root.dataset['theme'] = theme;
  installThemeStyles(root.ownerDocument);
}

/** The theme matching the operating system setting. */
export function systemTheme(): ThemeName {
  return window.matchMedia(DARK_QUERY).matches ? 'dark' : 'light';
}

/**
 * Applies the system theme now and whenever the system setting changes
 * (section 11: "standardmäßig passend zum System"). Returns a cleanup function.
 */
export function followSystemTheme(root: HTMLElement = document.documentElement): () => void {
  const query = window.matchMedia(DARK_QUERY);
  const update = () => {
    applyTheme(query.matches ? 'dark' : 'light', root);
  };
  update();
  query.addEventListener('change', update);
  return () => {
    query.removeEventListener('change', update);
  };
}
