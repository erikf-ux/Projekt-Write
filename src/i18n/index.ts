/**
 * Minimal translation layer: German and English dictionaries, language
 * following the system by default (section 11). No runtime dependency.
 */
import { create } from 'zustand';
import de from './de.json';
import en from './en.json';

export type Locale = 'de' | 'en';
export type MessageKey = keyof typeof de;

const dictionaries: Record<Locale, Record<MessageKey, string>> = { de, en };

/** Picks the first supported language from the system preference list. */
export function detectLocale(languages: readonly string[] = navigator.languages): Locale {
  for (const language of languages) {
    const primary = language.toLowerCase().split('-')[0];
    if (primary === 'de' || primary === 'en') {
      return primary;
    }
  }
  return 'en';
}

export function isMessageKey(key: string): key is MessageKey {
  return Object.hasOwn(de, key);
}

/** Translates `key`; `{name}` placeholders are replaced from `params`. */
export function translate(
  locale: Locale,
  key: MessageKey,
  params: Record<string, string | number> = {},
): string {
  const template = dictionaries[locale][key];
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
}

interface LocaleState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useLocale = create<LocaleState>((set) => ({
  locale: detectLocale(),
  setLocale: (locale) => {
    set({ locale });
  },
}));

/** React hook returning a translate function bound to the current locale. */
export function useT(): (key: MessageKey, params?: Record<string, string | number>) => string {
  const locale = useLocale((state) => state.locale);
  return (key, params) => translate(locale, key, params);
}

export { dictionaries };
