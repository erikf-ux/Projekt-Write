/**
 * State of the language check in the interface (section 14): the settings
 * (from the interface settings), a counter that rises when own
 * dictionaries change (cached results are then stale), words ignored for
 * this session, and the language profiles of open documents.
 */
import { create } from 'zustand';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import type { DocumentLanguage } from '@/types/generated/DocumentLanguage';
import type { LanguageCheckSettings } from '@/types/generated/LanguageCheckSettings';
import type { LanguageDictionary } from '@/types/generated/LanguageDictionary';

export const DEFAULT_CHECK_SETTINGS: LanguageCheckSettings = {
  enabled: true,
  rules: true,
  autocorrect: false,
  autocorrectAsked: false,
  defaultLanguage: null,
  detect: true,
};

interface LanguageStore {
  /** Rises with every change of own dictionaries or settings. */
  epoch: number;
  /** "Ignore" in the context menu, until the app closes. */
  ignored: ReadonlySet<string>;
  /** Language profiles of library documents, as far as loaded. */
  profiles: Readonly<Record<string, DocumentLanguage>>;
  dictionaries: LanguageDictionary[];
  bump: () => void;
  ignore: (word: string) => void;
  loadProfile: (documentId: string) => Promise<DocumentLanguage>;
  setProfile: (documentId: string, profile: DocumentLanguage) => Promise<boolean>;
  loadDictionaries: () => Promise<void>;
}

export const useLanguage = create<LanguageStore>((set, get) => ({
  epoch: 0,
  ignored: new Set(),
  profiles: {},
  dictionaries: [],
  bump: () => {
    set((state) => ({ epoch: state.epoch + 1 }));
  },
  ignore: (word) => {
    set((state) => ({ ignored: new Set([...state.ignored, word]), epoch: state.epoch + 1 }));
  },
  loadProfile: async (documentId) => {
    const known = get().profiles[documentId];
    if (known) {
      return known;
    }
    const result = await call('language_profile', { documentId });
    const profile = result.ok ? result.value : { languages: [], autocorrect: true };
    set((state) => ({ profiles: { ...state.profiles, [documentId]: profile } }));
    return profile;
  },
  setProfile: async (documentId, profile) => {
    const result = await call('language_profile_set', { documentId, profile });
    if (!result.ok) {
      return false;
    }
    set((state) => ({
      profiles: { ...state.profiles, [documentId]: result.value },
      epoch: state.epoch + 1,
    }));
    return true;
  },
  loadDictionaries: async () => {
    const result = await call('language_dictionaries', {});
    if (result.ok) {
      set({ dictionaries: result.value });
    }
  },
}));

/** The language check settings of the moment. */
export function checkSettings(): LanguageCheckSettings {
  return useUi.getState().state?.settings.languageCheck ?? DEFAULT_CHECK_SETTINGS;
}

/** The default language: the setting, or from the interface language. */
export function defaultLanguage(): string {
  const ui = useUi.getState().state;
  const setting = ui?.settings.languageCheck.defaultLanguage;
  if (setting) {
    return setting;
  }
  return ui?.locale === 'en' ? 'en-US' : 'de-DE';
}

/** The main language of a document: its profile, or the default. */
export function documentLanguage(documentId: string | null): string {
  if (documentId) {
    const profile = useLanguage.getState().profiles[documentId];
    const first = profile?.languages[0];
    if (first) {
      return first;
    }
  }
  return defaultLanguage();
}

// A changed setting makes every cached result stale.
useUi.subscribe((state, previous) => {
  const now = state.state?.settings.languageCheck;
  const before = previous.state?.settings.languageCheck;
  if (now !== before && JSON.stringify(now) !== JSON.stringify(before)) {
    useLanguage.getState().bump();
  }
});
