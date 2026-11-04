/**
 * Library documents in the interface: the Explorer list, the open tabs and
 * their text, and autosave (section 9).
 *
 * Autosave runs 1 s after the last input, on tab switch, on focus loss and
 * on close. Saves of one document never overlap; a save that fails keeps
 * the change in memory, the status bar shows "Fehler beim Speichern", and
 * the tab stays open. Library documents close without a question because
 * nothing is left unsaved.
 *
 * The text comes from the editor session of the tab (Phase 4), asked only
 * when saving. If saving as Markdown would lose something, the document
 * stays unsaved ("blocked") until the user saves anyway or changes it
 * (section 10, ADR 0014).
 */
import { create } from 'zustand';
import type { Loss } from '@/modules/editor/markdown/losses';
import type { ProbeIssue } from '@/modules/editor/markdown/probe';
import { closeAllSessions, closeSession, getSession, type SaveRequest } from '@/modules/editor/session';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { TextFormat } from '@/types/generated/TextFormat';

export const AUTOSAVE_DELAY_MS = 1000;

export type SaveState = 'saved' | 'pending' | 'saving' | 'error' | 'blocked';

/** Rich text, Markdown source, or plain text (`.txt`). */
export type DocumentView = 'rich' | 'source' | 'plain';

export interface OpenDocument {
  id: string;
  title: string;
  format: DocumentFormat;
  /** The text as read from the library; the editor session starts from it. */
  loadedText: string;
  /** The text as last saved. */
  savedText: string;
  /** Counts changes, so a save knows whether the text changed meanwhile. */
  revision: number;
  state: SaveState;
  error: CommandError | null;
  /** What saving as Markdown would lose (state "blocked"). */
  losses: Loss[];
  view: DocumentView;
  /** Why the rich view is not shown: at opening, or when switching to it. */
  viewIssues: ProbeIssue[];
  loaded: boolean;
}

/** Where a tab's text comes from when it is saved: its editor session. */
export interface DocumentSource {
  saveRequest: () => SaveRequest;
  acceptLosses: (losses: readonly Loss[]) => void;
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
  /** The text of the tab changed (the session has it). */
  edit: (id: string) => void;
  save: (id: string) => Promise<boolean>;
  /** "Trotzdem speichern": accept the losses shown and save. */
  saveAnyway: (id: string) => Promise<boolean>;
  /** Hides the loss warning; the document stays unsaved. */
  dismissLosses: (id: string) => void;
  /** Rich text ⇄ Markdown source (`Ctrl+/`). */
  toggleView: (id: string) => boolean;
  dismissViewIssues: (id: string) => void;
  /** The session of a tab exists: its view, and why it is not rich text. */
  attachView: (id: string, view: DocumentView, issues: ProbeIssue[]) => void;
  flush: (id: string) => Promise<boolean>;
  flushAll: () => Promise<boolean>;
  activate: (id: string) => void;
  closeTab: (id: string) => Promise<boolean>;
  closeOthers: (id: string) => Promise<void>;
  closeAll: () => Promise<void>;
  moveTab: (id: string, index: number) => void;
  rename: (id: string, title: string) => Promise<boolean>;
  setRenaming: (id: string | null) => void;
  /** Copies a document (saved first) and opens the copy. */
  copyDocument: (id: string, title: string) => Promise<string | null>;
  setFavorite: (id: string, favorite: boolean) => Promise<boolean>;
  /** Another library was opened: forget everything (after saving). */
  reset: () => Promise<void>;
  /** Adds a document created elsewhere (import) to the list. */
  addDocument: (summary: DocumentSummary) => void;
}

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const saving = new Map<string, Promise<boolean>>();
const sources = new Map<string, DocumentSource>();

/** The editor of a tab registers here once it exists. */
export function setDocumentSource(id: string, source: DocumentSource | null): void {
  if (source) {
    sources.set(id, source);
  } else {
    sources.delete(id);
  }
}

function newOpenDocument(id: string, title: string, format: DocumentFormat, loaded: boolean): OpenDocument {
  return {
    id,
    title,
    format,
    loadedText: '',
    savedText: '',
    revision: 0,
    state: 'saved',
    error: null,
    losses: [],
    view: format === 'md' ? 'rich' : 'plain',
    viewIssues: [],
    loaded,
  };
}

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
    const source = sources.get(id);
    if (!document?.loaded || !source) {
      return true;
    }
    const request = source.saveRequest();
    if (request.kind === 'blocked') {
      patch(id, { state: 'blocked', losses: request.losses, error: null });
      return false;
    }
    const { text } = request;
    const revision = document.revision;
    if (text === document.savedText) {
      patch(id, { state: 'saved', error: null, losses: [] });
      return true;
    }
    patch(id, { state: 'saving', losses: [] });
    const result = await call('document_save', { id, text });
    const now = get().open[id];
    if (!result.ok) {
      patch(id, { state: 'error', error: result.error });
      set({ lastError: result.error });
      return false;
    }
    set((state) => ({ documents: upsert(state.documents, result.value) }));
    if (now) {
      const unchangedSince = now.revision === revision;
      patch(id, { savedText: text, state: unchangedSince ? 'saved' : 'pending', error: null });
      if (!unchangedSince) {
        schedule(id);
      }
    }
    return true;
  };

  const removeTab = (id: string) => {
    clearTimer(id);
    sources.delete(id);
    closeSession(id);
    set((state) => {
      const index = state.tabs.indexOf(id);
      const tabs = state.tabs.filter((tab) => tab !== id);
      const open = Object.fromEntries(Object.entries(state.open).filter(([key]) => key !== id));
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
          [id]: newOpenDocument(id, summary?.title ?? '', summary?.format ?? 'txt', false),
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
      patch(id, {
        title: document.title,
        format: document.format,
        loadedText: text,
        savedText: text,
        view: document.format === 'md' ? 'rich' : 'plain',
        loaded: true,
      });
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
          [document.id]: newOpenDocument(document.id, document.title, document.format, true),
        },
      }));
      get().activate(document.id);
      return document.id;
    },

    edit: (id) => {
      const document = get().open[id];
      if (!document?.loaded) {
        return;
      }
      patch(id, { state: 'pending', error: null, revision: document.revision + 1 });
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

    saveAnyway: async (id) => {
      const document = get().open[id];
      const source = sources.get(id);
      if (!document || !source) {
        return true;
      }
      source.acceptLosses(document.losses);
      return get().save(id);
    },

    dismissLosses: (id) => {
      patch(id, { losses: [] });
    },

    toggleView: (id) => {
      const session = getSession(id);
      if (!session?.isMarkdown) {
        return false;
      }
      const result = session.toggleView();
      patch(id, { view: session.view, viewIssues: result.issues });
      return result.ok;
    },

    attachView: (id, view, issues) => {
      patch(id, { view, viewIssues: issues });
    },

    dismissViewIssues: (id) => {
      patch(id, { viewIssues: [] });
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

    copyDocument: async (id, title) => {
      // The copy takes what is on disk: unsaved text is saved first.
      if (get().open[id] && !(await get().flush(id))) {
        return null;
      }
      const result = await call('document_copy', { id, title });
      if (!result.ok) {
        set({ lastError: result.error });
        return null;
      }
      set((state) => ({ documents: upsert(state.documents, result.value) }));
      await get().openDocument(result.value.id);
      return result.value.id;
    },

    setFavorite: async (id, favorite) => {
      const result = await call('document_set_favorite', { id, favorite });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set((state) => ({ documents: upsert(state.documents, result.value) }));
      return true;
    },

    reset: async () => {
      await get().flushAll();
      for (const id of get().tabs) {
        clearTimer(id);
      }
      sources.clear();
      closeAllSessions();
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
  while (titles.has(`${base} ${String(number)}`)) {
    number += 1;
  }
  return `${base} ${String(number)}`;
}
