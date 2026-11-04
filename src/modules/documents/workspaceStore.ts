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
 *
 * External files (Phase 5, ADR 0015) open in tabs `ext:<id>`. They are
 * saved with Ctrl+S, automatically only if the setting says so; closing a
 * tab with changes asks "Speichern, Verwerfen, Abbrechen". Their encoding
 * and line endings stay as they were unless the user changes them. A
 * change from outside is reported by `checkExternalFiles` and decided in a
 * dialog: reload, keep my version, or save as a copy.
 */
import { create } from 'zustand';
import { ask } from '@/components/prompt/promptStore';
import type { Loss } from '@/modules/editor/markdown/losses';
import type { ProbeIssue } from '@/modules/editor/markdown/probe';
import { closeAllSessions, closeSession, getSession, type SaveRequest } from '@/modules/editor/session';
import { useUi } from '@/modules/settings/uiStore';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { ExternalFileInfo } from '@/types/generated/ExternalFileInfo';
import type { ExternalText } from '@/types/generated/ExternalText';
import type { LineEnding } from '@/types/generated/LineEnding';
import type { RecentFileEntry } from '@/types/generated/RecentFileEntry';
import type { TextEncoding } from '@/types/generated/TextEncoding';
import type { TextFormat } from '@/types/generated/TextFormat';

export const AUTOSAVE_DELAY_MS = 1000;

export type SaveState = 'saved' | 'pending' | 'saving' | 'error' | 'blocked';

/** Rich text, Markdown source, or plain text (`.txt`). */
export type DocumentView = 'rich' | 'source' | 'plain';

/** A file outside the library, open in a tab. */
export interface ExternalState {
  /** Id Rust gave the file. */
  fileId: string;
  name: string;
  /** Display text only. */
  folder: string;
  /** What the next save writes. */
  encoding: TextEncoding;
  lineEnding: LineEnding;
  /** What the file has. */
  savedEncoding: TextEncoding;
  savedLineEnding: LineEnding;
  /** The file mixes line endings; the first save makes them uniform. */
  mixedLineEndings: boolean;
  /** The file cannot be written (permissions). */
  fileReadOnly: boolean;
  /** Changed or gone outside the app, not decided yet. */
  outside: 'changed' | 'missing' | null;
  /** The tab text is not what the file holds although it equals `savedText`. */
  differsFromFile: boolean;
}

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
  /** Shown but never written (a `.pwrite` file of a newer format version). */
  readOnly: boolean;
  /** Counts reloads from disk; the editor starts anew with each. */
  generation: number;
  /** Set for files outside the library. */
  external: ExternalState | null;
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
  /** External files opened last (newest first). */
  recentFiles: RecentFileEntry[];
  /** Shows the open dialog and opens the chosen file. */
  openFile: (dialogTitle: string) => Promise<boolean>;
  /** Opens a file Rust registered (dialog, recent list, start argument). */
  openExternal: (info: ExternalFileInfo) => Promise<boolean>;
  openRecentFile: (recentId: string, name: string) => Promise<boolean>;
  forgetRecentFile: (recentId: string) => Promise<void>;
  refreshRecentFiles: () => Promise<void>;
  /** "Speichern unter" for an external tab. */
  saveAs: (id: string, dialogTitle: string) => Promise<boolean>;
  /** "Neu laden" after a change from outside: the file's text replaces the tab's. */
  reloadExternal: (id: string) => Promise<boolean>;
  /** "Meine Fassung behalten": the next save writes the tab's text over the file. */
  keepMine: (id: string) => Promise<boolean>;
  setEncoding: (id: string, encoding: TextEncoding) => void;
  setLineEnding: (id: string, lineEnding: LineEnding) => void;
  /** Asks Rust whether external files changed outside the app. */
  checkExternalFiles: () => Promise<void>;
  /** "In Bibliothek aufnehmen": the tab then shows the library document. */
  takeIntoLibrary: (id: string) => Promise<string | null>;
  /** Before quitting: asks for each external tab with changes. False: stay. */
  confirmUnsaved: () => Promise<boolean>;
}

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const saving = new Map<string, Promise<boolean>>();
const sources = new Map<string, DocumentSource>();
/** Counts saves of external tabs, so a check that crossed a save is ignored. */
const saveEpochs = new Map<string, number>();

/** Tab id of an external file. */
export function externalTabId(fileId: string): string {
  return `ext:${fileId}`;
}

function autosaveExternal(): boolean {
  return useUi.getState().state?.settings.autosaveExternal ?? false;
}

/** Whether a tab holds changes that are neither in its file nor its document. */
export function hasUnsavedChanges(document: OpenDocument): boolean {
  if (!document.loaded || document.readOnly) {
    return false;
  }
  const { external } = document;
  if (
    external &&
    (external.differsFromFile ||
      external.encoding !== external.savedEncoding ||
      external.lineEnding !== external.savedLineEnding)
  ) {
    return true;
  }
  if (document.state === 'saved') {
    return false;
  }
  const source = sources.get(document.id);
  if (!source) {
    return true;
  }
  const request = source.saveRequest();
  return request.kind === 'blocked' || request.text !== document.savedText;
}

function askUnsaved(document: OpenDocument): Promise<'save' | 'discard' | 'cancel'> {
  return ask({
    titleKey: 'external.unsaved.title',
    textKey: 'external.unsaved.text',
    params: { name: document.title },
    choices: [
      { id: 'save', labelKey: 'external.unsaved.save', tone: 'primary' },
      { id: 'discard', labelKey: 'external.unsaved.discard', tone: 'danger' },
      { id: 'cancel', labelKey: 'external.unsaved.cancel' },
    ],
    cancel: 'cancel',
    testId: 'unsaved-prompt',
  });
}

function newExternalState(info: ExternalFileInfo): ExternalState {
  return {
    fileId: info.id,
    name: info.name,
    folder: info.folder,
    encoding: 'utf8',
    lineEnding: 'lf',
    savedEncoding: 'utf8',
    savedLineEnding: 'lf',
    mixedLineEndings: false,
    fileReadOnly: false,
    outside: null,
    differsFromFile: false,
  };
}

/** The state of an external tab after reading its file. */
function loadedExternal(text: ExternalText): Partial<ExternalState> {
  return {
    encoding: text.encoding,
    lineEnding: text.lineEnding,
    savedEncoding: text.encoding,
    savedLineEnding: text.lineEnding,
    mixedLineEndings: text.mixedLineEndings,
    fileReadOnly: text.readOnly && !text.newerFormat,
    outside: null,
    differsFromFile: false,
  };
}

function megabytes(size: number): string {
  return (size / (1024 * 1024)).toFixed(1);
}

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
    view: format === 'txt' ? 'plain' : 'rich',
    viewIssues: [],
    loaded,
    readOnly: false,
    generation: 0,
    external: null,
  };
}

/** The content of a new `.pwrite` document, as Rust creates it. */
export const EMPTY_PWRITE_TEXT = JSON.stringify({ type: 'doc', content: [{ type: 'paragraph' }] });

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

  const patchExternal = (id: string, change: Partial<ExternalState>) => {
    set((state) => {
      const current = state.open[id];
      return current?.external
        ? { open: { ...state.open, [id]: { ...current, external: { ...current.external, ...change } } } }
        : {};
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
    if (document.external) {
      return writeExternal(id, document, document.external, text);
    }
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

  const writeExternal = async (
    id: string,
    document: OpenDocument,
    external: ExternalState,
    text: string,
  ): Promise<boolean> => {
    const revision = document.revision;
    const { encoding, lineEnding } = external;
    if (
      text === document.savedText &&
      !external.differsFromFile &&
      encoding === external.savedEncoding &&
      lineEnding === external.savedLineEnding
    ) {
      // Unchanged files are never written (ADR 0015).
      patch(id, { state: 'saved', error: null, losses: [] });
      return true;
    }
    if (external.outside) {
      // The dialog about the change from outside decides first.
      patch(id, { state: 'pending', losses: [] });
      return false;
    }
    saveEpochs.set(id, (saveEpochs.get(id) ?? 0) + 1);
    patch(id, { state: 'saving', losses: [] });
    const result = await call('external_save', {
      id: external.fileId,
      text,
      encoding,
      lineEnding,
      overwrite: false,
    });
    if (!result.ok) {
      const { code } = result.error;
      if (code === 'FILE_CHANGED' || code === 'FILE_MISSING') {
        patchExternal(id, { outside: code === 'FILE_CHANGED' ? 'changed' : 'missing' });
        patch(id, { state: 'pending', error: null });
      } else {
        patch(id, { state: 'error', error: result.error });
        if (code === 'FILE_READ_ONLY') {
          patchExternal(id, { fileReadOnly: true });
        }
      }
      return false;
    }
    const now = get().open[id];
    if (now) {
      const unchangedSince = now.revision === revision;
      patch(id, { savedText: text, state: unchangedSince ? 'saved' : 'pending', error: null });
      patchExternal(id, {
        savedEncoding: encoding,
        savedLineEnding: lineEnding,
        differsFromFile: false,
        mixedLineEndings: false,
      });
      if (!unchangedSince && autosaveExternal()) {
        schedule(id);
      }
    }
    return true;
  };

  /** A change to the tab that is not in its text (encoding, line ending). */
  const touchExternal = (id: string) => {
    const document = get().open[id];
    if (!document) {
      return;
    }
    patch(id, { revision: document.revision + 1, error: null });
    const updated = get().open[id];
    if (updated && !hasUnsavedChanges(updated)) {
      patch(id, { state: 'saved' });
      return;
    }
    patch(id, { state: 'pending' });
    if (autosaveExternal()) {
      schedule(id);
    }
  };

  const closeExternalFile = (fileId: string) => {
    void call('external_close', { id: fileId });
  };

  const removeTab = (id: string) => {
    clearTimer(id);
    sources.delete(id);
    saveEpochs.delete(id);
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
    recentFiles: [],

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
        view: document.format === 'txt' ? 'plain' : 'rich',
        loaded: true,
        readOnly: result.value.readOnly,
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
      const opened = newOpenDocument(document.id, document.title, document.format, true);
      if (document.format === 'pwrite') {
        opened.loadedText = EMPTY_PWRITE_TEXT;
        opened.savedText = EMPTY_PWRITE_TEXT;
      }
      set((state) => ({
        documents: upsert(state.documents, document),
        tabs: [...state.tabs, document.id],
        open: {
          ...state.open,
          [document.id]: opened,
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
      if (!document.external || autosaveExternal()) {
        schedule(id);
      }
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
      if (document.external && !autosaveExternal()) {
        // Saved with Ctrl+S; closing asks.
        return (await saving.get(id)) ?? true;
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
      const document = get().open[id];
      if (document?.external) {
        if (hasUnsavedChanges(document)) {
          get().activate(id);
          const answer = await askUnsaved(document);
          if (answer === 'cancel' || (answer === 'save' && !(await get().save(id)))) {
            return false;
          }
        }
        removeTab(id);
        closeExternalFile(document.external.fileId);
        return true;
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
      // External files do not belong to the library; their tabs stay.
      const { tabs, open, active } = get();
      const kept = tabs.filter((id) => open[id]?.external);
      for (const id of tabs) {
        if (!kept.includes(id)) {
          clearTimer(id);
          sources.delete(id);
          closeSession(id);
        }
      }
      if (kept.length === 0) {
        closeAllSessions();
      }
      set({
        documents: [],
        listLoaded: false,
        listError: null,
        tabs: kept,
        active: active !== null && kept.includes(active) ? active : (kept[0] ?? null),
        open: Object.fromEntries(kept.flatMap((id) => (open[id] ? [[id, open[id]]] : []))),
        renaming: null,
      });
    },

    addDocument: (summary) => {
      set((state) => ({ documents: upsert(state.documents, summary) }));
    },

    openFile: async (dialogTitle) => {
      const result = await call('external_open', { title: dialogTitle });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      return result.value ? get().openExternal(result.value) : false;
    },

    openExternal: async (info) => {
      const tab = externalTabId(info.id);
      if (get().open[tab]) {
        get().activate(tab);
        return true;
      }
      if (info.large) {
        const answer = await ask({
          titleKey: 'external.large.title',
          textKey: 'external.large.text',
          params: { name: info.name, size: megabytes(info.size) },
          choices: [
            { id: 'open', labelKey: 'external.large.open', tone: 'primary' },
            { id: 'cancel', labelKey: 'external.large.cancel' },
          ],
          cancel: 'cancel',
          testId: 'large-file-prompt',
        });
        if (answer !== 'open') {
          closeExternalFile(info.id);
          return false;
        }
      }
      const opened: OpenDocument = {
        ...newOpenDocument(tab, info.name, info.format, false),
        external: newExternalState(info),
      };
      set((state) => ({ tabs: [...state.tabs, tab], open: { ...state.open, [tab]: opened } }));
      get().activate(tab);
      const result = await call('external_read', { id: info.id });
      if (!result.ok) {
        removeTab(tab);
        closeExternalFile(info.id);
        set({ lastError: result.error });
        return false;
      }
      const text = result.value;
      patch(tab, {
        loadedText: text.text,
        savedText: text.text,
        loaded: true,
        readOnly: text.newerFormat,
      });
      patchExternal(tab, loadedExternal(text));
      void get().refreshRecentFiles();
      return true;
    },

    openRecentFile: async (recentId, name) => {
      const result = await call('external_open_recent', { id: recentId });
      if (result.ok) {
        return get().openExternal(result.value);
      }
      if (result.error.code === 'FILE_MISSING' || result.error.code === 'FILE_UNKNOWN') {
        const answer = await ask({
          titleKey: 'external.missingRecent.title',
          textKey: 'external.missingRecent.text',
          params: { name },
          choices: [
            { id: 'remove', labelKey: 'external.missingRecent.remove', tone: 'primary' },
            { id: 'keep', labelKey: 'external.missingRecent.keep' },
          ],
          cancel: 'keep',
          testId: 'missing-recent-prompt',
        });
        if (answer === 'remove') {
          await get().forgetRecentFile(recentId);
        }
      } else {
        set({ lastError: result.error });
      }
      return false;
    },

    forgetRecentFile: async (recentId) => {
      const result = await call('external_forget_recent', { id: recentId });
      if (result.ok) {
        set({ recentFiles: result.value });
      } else {
        set({ lastError: result.error });
      }
    },

    refreshRecentFiles: async () => {
      const result = await call('external_recent', {});
      if (result.ok) {
        set({ recentFiles: result.value });
      }
    },

    saveAs: async (id, dialogTitle) => {
      const document = get().open[id];
      const source = sources.get(id);
      const external = document?.external;
      if (!document?.loaded || !external || !source || document.readOnly) {
        return false;
      }
      const request = source.saveRequest();
      if (request.kind === 'blocked') {
        patch(id, { state: 'blocked', losses: request.losses, error: null });
        return false;
      }
      const { encoding, lineEnding } = external;
      const revision = document.revision;
      saveEpochs.set(id, (saveEpochs.get(id) ?? 0) + 1);
      const result = await call('external_save_as', {
        id: external.fileId,
        title: dialogTitle,
        text: request.text,
        encoding,
        lineEnding,
      });
      if (!result.ok) {
        patch(id, { state: 'error', error: result.error });
        return false;
      }
      const info = result.value;
      if (!info) {
        return false;
      }
      const unchangedSince = get().open[id]?.revision === revision;
      patch(id, {
        title: info.name,
        savedText: request.text,
        state: unchangedSince ? 'saved' : 'pending',
        error: null,
        losses: [],
      });
      patchExternal(id, {
        name: info.name,
        folder: info.folder,
        savedEncoding: encoding,
        savedLineEnding: lineEnding,
        mixedLineEndings: false,
        fileReadOnly: false,
        outside: null,
        differsFromFile: false,
      });
      void get().refreshRecentFiles();
      return true;
    },

    reloadExternal: async (id) => {
      const document = get().open[id];
      const external = document?.external;
      if (!document || !external) {
        return false;
      }
      const result = await call('external_read', { id: external.fileId });
      if (!result.ok) {
        if (result.error.code === 'FILE_MISSING') {
          patchExternal(id, { outside: 'missing' });
        }
        set({ lastError: result.error });
        return false;
      }
      const text = result.value;
      clearTimer(id);
      sources.delete(id);
      closeSession(id);
      const current = get().open[id] ?? document;
      patch(id, {
        loadedText: text.text,
        savedText: text.text,
        state: 'saved',
        error: null,
        losses: [],
        viewIssues: [],
        readOnly: text.newerFormat,
        revision: current.revision + 1,
        generation: current.generation + 1,
      });
      patchExternal(id, loadedExternal(text));
      return true;
    },

    keepMine: async (id) => {
      const document = get().open[id];
      const external = document?.external;
      if (!document || !external) {
        return false;
      }
      if (external.outside === 'changed') {
        const result = await call('external_keep_mine', { id: external.fileId });
        if (!result.ok) {
          if (result.error.code === 'FILE_MISSING') {
            patchExternal(id, { outside: 'missing' });
          }
          set({ lastError: result.error });
          return false;
        }
      }
      patchExternal(id, { outside: null, differsFromFile: true });
      touchExternal(id);
      return true;
    },

    setEncoding: (id, encoding) => {
      if (!get().open[id]?.external) {
        return;
      }
      patchExternal(id, { encoding });
      touchExternal(id);
    },

    setLineEnding: (id, lineEnding) => {
      if (!get().open[id]?.external) {
        return;
      }
      patchExternal(id, { lineEnding });
      touchExternal(id);
    },

    checkExternalFiles: async () => {
      for (const id of get().tabs) {
        const document = get().open[id];
        const external = document?.external;
        if (!document?.loaded || !external || external.outside || saving.has(id)) {
          continue;
        }
        const epoch = saveEpochs.get(id) ?? 0;
        const result = await call('external_check', { id: external.fileId });
        const now = get().open[id]?.external;
        if (!result.ok || !now || now.outside || saving.has(id) || (saveEpochs.get(id) ?? 0) !== epoch) {
          continue;
        }
        const { status, readOnly } = result.value;
        if (status !== 'unchanged') {
          patchExternal(id, { outside: status });
        } else if (readOnly !== now.fileReadOnly && !document.readOnly) {
          patchExternal(id, { fileReadOnly: readOnly });
        }
      }
    },

    takeIntoLibrary: async (id) => {
      const document = get().open[id];
      const source = sources.get(id);
      const external = document?.external;
      if (!document?.loaded || !external || !source) {
        return null;
      }
      const request = source.saveRequest();
      if (request.kind === 'blocked') {
        patch(id, { state: 'blocked', losses: request.losses, error: null });
        return null;
      }
      let allowDuplicate = false;
      let target: DocumentSummary | null = null;
      while (!target) {
        const result = await call('external_import', {
          id: external.fileId,
          text: request.text,
          allowDuplicate,
        });
        if (!result.ok) {
          set({ lastError: result.error });
          return null;
        }
        if (result.value.kind === 'imported') {
          target = result.value.document;
          break;
        }
        const existing = result.value.existing;
        const answer = await ask({
          titleKey: 'external.duplicate.title',
          textKey: 'external.duplicate.text',
          params: { title: existing.title },
          choices: [
            { id: 'existing', labelKey: 'external.duplicate.openExisting', tone: 'primary' },
            { id: 'import', labelKey: 'external.duplicate.importAnyway' },
            { id: 'cancel', labelKey: 'external.duplicate.cancel' },
          ],
          cancel: 'cancel',
          testId: 'duplicate-prompt',
        });
        if (answer === 'cancel') {
          return null;
        }
        if (answer === 'existing') {
          target = existing;
        }
        allowDuplicate = true;
      }
      const summary = target;
      set((state) => ({ documents: upsert(state.documents, summary) }));
      // The tab shows the library document now, at the same place.
      const index = get().tabs.indexOf(id);
      removeTab(id);
      closeExternalFile(external.fileId);
      if (!(await get().openDocument(summary.id))) {
        return null;
      }
      get().moveTab(summary.id, index);
      return summary.id;
    },

    confirmUnsaved: async () => {
      for (const id of [...get().tabs]) {
        const document = get().open[id];
        if (!document?.external || !hasUnsavedChanges(document)) {
          continue;
        }
        get().activate(id);
        const answer = await askUnsaved(document);
        if (answer === 'cancel' || (answer === 'save' && !(await get().save(id)))) {
          return false;
        }
      }
      return true;
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
