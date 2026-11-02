/**
 * Project Write design tokens (master prompt, section 11).
 *
 * This is the single source of truth for colours. `applyTheme` turns the
 * tokens into CSS custom properties; Tailwind utilities reference those
 * properties (see `src/index.css`). Contrast is verified in `tokens.test.ts`.
 */

export type ThemeName = 'dark' | 'light';

export interface ThemeTokens {
  /** Writing surface and main background. */
  surface: string;
  /** Side bar, cards and the status bar. */
  sidebar: string;
  text: string;
  textMuted: string;
  accent: string;
  error: string;
  /** Positive status (not in the section 11 table; same values as Cluster Green). */
  success: string;
  clusterBlue: string;
  clusterGreen: string;
  clusterViolet: string;
  clusterOrange: string;
  clusterGrey: string;
}

export const themes: Record<ThemeName, ThemeTokens> = {
  dark: {
    surface: '#1E1E1E',
    sidebar: '#252526',
    text: '#D4D4D4',
    textMuted: '#A0A4AA',
    accent: '#6C9BFF',
    error: '#F48771',
    success: '#4EC97A',
    clusterBlue: '#6C9BFF',
    clusterGreen: '#4EC97A',
    clusterViolet: '#B58CFF',
    clusterOrange: '#F0A050',
    clusterGrey: '#9AA0A6',
  },
  light: {
    surface: '#FFFFFF',
    sidebar: '#F3F3F3',
    text: '#1F1F1F',
    textMuted: '#5F6368',
    accent: '#2F5FE0',
    error: '#C72E0F',
    success: '#1B7F3B',
    clusterBlue: '#2F5FE0',
    clusterGreen: '#1B7F3B',
    clusterViolet: '#7A4FD0',
    clusterOrange: '#B85C00',
    clusterGrey: '#6B7075',
  },
};

/** CSS custom property for each token. */
export const tokenVariables: Record<keyof ThemeTokens, string> = {
  surface: '--pw-color-surface',
  sidebar: '--pw-color-sidebar',
  text: '--pw-color-text',
  textMuted: '--pw-color-text-muted',
  accent: '--pw-color-accent',
  error: '--pw-color-error',
  success: '--pw-color-success',
  clusterBlue: '--pw-color-cluster-blue',
  clusterGreen: '--pw-color-cluster-green',
  clusterViolet: '--pw-color-cluster-violet',
  clusterOrange: '--pw-color-cluster-orange',
  clusterGrey: '--pw-color-cluster-grey',
};
