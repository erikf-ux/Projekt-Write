/**
 * Library documents in the interface: the Explorer list, the open tabs and
 * their text, and autosave (section 9).
 *
 * Autosave runs 1 s after the last input, on tab switch, on focus loss and
 * on close. Saves of one document never overlap; a save that fails keeps
 * the change in memory, the status bar shows "Fehler beim Speichern", and
 * the tab stays open. Library documents close without a question because
 * nothing is left unsaved.
 */
import { create } from 'zustand';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { TextFormat } from '@/types/generated/TextFormat';

export const AUTOSAVE_DELAY_MS = 1000;

export type SaveState = 'saved' | 'pending' | 'saving' | 'error';

export interface OpenDocument {
  id: string;
  title: string;
  format: DocumentFormat;
  /** The text in the writing surface. */
  text: string;
  /** The text as last saved. */
  savedText: string;
  state: SaveState;
  error: CommandError | null;
  loaded: boolean;
}

interface WorkspaceStore {
  documents: DocumentSummary[];
  listLoaded: boolean;
  listError: CommandError | null;
  tabs: string[];
  active: string | null;
  open: Record<string, OpenDocument>;
  /** Document whose title is being edited in the Explorer. */
  renaming: string | null;
  lastError: CommandError | null;
  refresh: () => Promise<void>;
  openDocument: (id: string) => Promise<boolean>;
  createDocument: (title: string, format?: TextFormat) => Promise<string | null>;
  edit: (id: string, text: string) => void;
  save: (id: string) => Promise<boolean>;
  flush: (id: string) => Promise<boolean>;
  flushAll: () => Promise<boolean>;
  activate: (id: string) => void;
  closeTab: (id: string) => Promise<boolean>;
  closeOthers: (id: string) => Promise<void>;
  closeAll: () => Promise<void>;
  moveTab: (id: string, index: number) => void;
  rename: (id: string, title: string) => Promise<boolean>;
  setRenaming: (id: string | null) => void;
  /** Another library was opened: forget everything (after saving). */
  reset: () => Promise<void>;
  /** Adds a document created elsewhere (import) to the list. */
  addDocument: (summary: DocumentSummary) => void;
}

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const saving = new Map<string, Promise<boolean>>();

function clearTimer(id: string): void {
  const timer = timers.get(id);
  if (timer !== undefined) {
    clearTimeout(timer);
    timers.delete(id);
  }
}

function upsert(list: readonly DocumentSummary[], summary: DocumentSummary): DocumentSummary[] {
  const index = list.findIndex((entry) => entry.id === summary.id);
  if (index === -1) {
    return [summary, ...list];
  }
  const next = [...list];
  next[index] = summary;
  return next;
}

export const useWorkspace = create<WorkspaceStore>((set, get) => {
  const patch = (id: string, change: Partial<OpenDocument>) => {
    set((state) => {
      const current = state.open[id];
      return current ? { open: { ...state.open, [id]: { ...current, ...change } } } : {};
    });
  };

  const schedule = (id: string) => {
    clearTimer(id);
    timers.set(
      id,
      setTimeout(() => {
        timers.delete(id);
        void get().save(id);
      }, AUTOSAVE_DELAY_MS),
    );
  };

  /** One save; callers serialise through `save`. */
  const write = async (id: string): Promise<boolean> => {
    clearTimer(id);
    const document = get().open[id];
    if (!document?.loaded) {
      return true;
    }
    if (document.text === document.savedText) {
      patch(id, { state: 'saved', error: null });
      return true;
    }
    const text = document.text;
    patch(id, { state: 'saving' });
    const result = await call('document_save', { id, text });
    const now = get().open[id];
    if (!result.ok) {
      patch(id, { state: 'error', error: result.error });
      set({ lastError: result.error });
      return false;
    }
    set((state) => ({ documents: upsert(state.documents, result.value) }));
    if (now) {
      const unchangedSince = now.text === text;
      patch(id, { savedText: text, state: unchangedSince ? 'saved' : 'pending', error: null });
      if (!unchangedSince) {
        schedule(id);
      }
    }
    return true;
  };

  const removeTab = (id: string) => {
    clearTimer(id);
    set((state) => {
      const index = state.tabs.indexOf(id);
      const tabs = state.tabs.filter((tab) => tab !== id);
      const open = { ...state.open };
      delete open[id];
      let active = state.active;
      if (active === id) {
        active = tabs[Math.min(index, tabs.length - 1)] ?? null;
      }
      return { tabs, open, active, renaming: state.renaming === id ? null : state.renaming };
    });
  };

  return {
    documents: [],
    listLoaded: false,
    listError: null,
    tabs: [],
    active: null,
    open: {},
    renaming: null,
    lastError: null,

    refresh: async () => {
      const result = await call('documents_list', {});
      if (result.ok) {
        set({ documents: result.value, listLoaded: true, listError: null });
      } else {
        set({ listError: result.error, listLoaded: true });
      }
    },

    openDocument: async (id) => {
      if (get().open[id]) {
        get().activate(id);
        return true;
      }
      const summary = get().documents.find((entry) => entry.id === id);
      set((state) => ({
        tabs: [...state.tabs, id],
        open: {
          ...state.open,
          [id]: {
            id,
            title: summary?.title ?? '',
            format: summary?.format ?? 'txt',
            text: '',
            savedText: '',
            state: 'saved',
            error: null,
            loaded: false,
          },
        },
      }));
      get().activate(id);
      const result = await call('document_read', { id });
      if (!result.ok) {
        removeTab(id);
        set({ lastError: result.error });
        return false;
      }
      const { document, text } = result.value;
      patch(id, { title: document.title, format: document.format, text, savedText: text, loaded: true });
      set((state) => ({ documents: upsert(state.documents, document) }));
      return true;
    },

    createDocument: async (title, format = 'md') => {
      const result = await call('document_create', { title, format });
      if (!result.ok) {
        set({ lastError: result.error });
        return null;
      }
      const document = result.value;
      set((state) => ({
        documents: upsert(state.documents, document),
        tabs: [...state.tabs, document.id],
        open: {
          ...state.open,
          [document.id]: {
            id: document.id,
            title: document.title,
            format: document.format,
            text: '',
            savedText: '',
            state: 'saved',
            error: null,
            loaded: true,
          },
        },
      }));
      get().activate(document.id);
      return document.id;
    },

    edit: (id, text) => {
      const document = get().open[id];
      if (!document?.loaded) {
        return;
      }
      patch(id, { text, state: text === document.savedText ? 'saved' : 'pending', error: null });
      schedule(id);
    },

    save: async (id) => {
      const previous = saving.get(id) ?? Promise.resolve(true);
      const next = previous.then(() => write(id));
      saving.set(id, next);
      try {
        return await next;
      } finally {
        if (saving.get(id) === next) {
          saving.delete(id);
        }
      }
    },

    flush: async (id) => {
      const document = get().open[id];
      if (!document) {
        return true;
      }
      if (document.state === 'saved' && !saving.has(id)) {
        return true;
      }
      return get().save(id);
    },

    flushAll: async () => {
      const results = await Promise.all(get().tabs.map((id) => get().flush(id)));
      return results.every(Boolean);
    },

    activate: (id) => {
      const previous = get().active;
      if (previous === id || !get().open[id]) {
        return;
      }
      set({ active: id });
      if (previous) {
        void get().flush(previous);
      }
    },

    closeTab: async (id) => {
      if (!(await get().flush(id))) {
        return false;
      }
      removeTab(id);
      return true;
    },

    closeOthers: async (id) => {
      for (const tab of get().tabs.filter((other) => other !== id)) {
        await get().closeTab(tab);
      }
    },

    closeAll: async () => {
      for (const tab of [...get().tabs]) {
        await get().closeTab(tab);
      }
    },

    moveTab: (id, index) => {
      set((state) => {
        const from = state.tabs.indexOf(id);
        if (from === -1) {
          return {};
        }
        const tabs = state.tabs.filter((tab) => tab !== id);
        tabs.splice(Math.max(0, Math.min(index, tabs.length)), 0, id);
        return { tabs };
      });
    },

    rename: async (id, title) => {
      const result = await call('document_rename', { id, title });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set((state) => ({ documents: upsert(state.documents, result.value), renaming: null }));
      patch(id, { title: result.value.title });
      return true;
    },

    setRenaming: (id) => {
      set({ renaming: id });
    },

    reset: async () => {
      await get().flushAll();
      for (const id of get().tabs) {
        clearTimer(id);
      }
      set({
        documents: [],
        listLoaded: false,
        listError: null,
        tabs: [],
        active: null,
        open: {},
        renaming: null,
      });
    },

    addDocument: (summary) => {
      set((state) => ({ documents: upsert(state.documents, summary) }));
    },
  };
});

/** The open document of the active tab. */
export function activeDocument(state: Pick<WorkspaceStore, 'active' | 'open'>): OpenDocument | null {
  return state.active ? (state.open[state.active] ?? null) : null;
}

/** A title like "Unbenannt" that no document has yet ("Unbenannt 2", …). */
export function freeTitle(base: string, documents: readonly DocumentSummary[]): string {
  const titles = new Set(documents.map((document) => document.title));
  if (!titles.has(base)) {
    return base;
  }
  let number = 2;
  while (titles.has(`${base} ${number}`)) {
    number += 1;
  }
  return `${base} ${number}`;
}
