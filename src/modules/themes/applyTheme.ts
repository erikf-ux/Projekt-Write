import { themes, tokenVariables, type ThemeName } from './tokens';

const DARK_QUERY = '(prefers-color-scheme: dark)';

/** Writes the tokens of `theme` as CSS custom properties onto `root`. */
export function applyTheme(theme: ThemeName, root: HTMLElement = document.documentElement): void {
  const tokens = themes[theme];
  for (const [token, variable] of Object.entries(tokenVariables) as [keyof typeof tokens, string][]) {
    root.style.setProperty(variable, tokens[token]);
  }
  root.dataset['theme'] = theme;
  root.style.colorScheme = theme;
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
