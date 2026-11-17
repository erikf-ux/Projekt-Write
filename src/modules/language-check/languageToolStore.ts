/**
 * The LanguageTool add-on as the interface sees it (section 14, ADR 0026):
 * its status, watched while the server starts. Once it runs, every cached
 * result is stale (the paragraphs get its findings too).
 */
import { create } from 'zustand';
import { call } from '@/services/ipc';
import type { LanguageToolStatus } from '@/types/generated/LanguageToolStatus';
import { useLanguage } from './languageStore';

const WATCH_MS = 2000;

interface LanguageToolStore {
  status: LanguageToolStatus | null;
  /** Asks Rust (which starts the server when it is set up and on). */
  refresh: () => Promise<void>;
  apply: (status: LanguageToolStatus) => void;
}

let timer: ReturnType<typeof setTimeout> | null = null;

export const useLanguageTool = create<LanguageToolStore>((set, get) => ({
  status: null,
  refresh: async () => {
    const result = await call('languagetool_status', {});
    if (result.ok) {
      get().apply(result.value);
    }
  },
  apply: (status) => {
    const before = get().status?.state;
    set({ status });
    if (before !== status.state && (status.state === 'running' || before === 'running')) {
      useLanguage.getState().bump();
    }
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (status.state === 'starting') {
      timer = setTimeout(() => {
        timer = null;
        void get().refresh();
      }, WATCH_MS);
    }
  },
}));
