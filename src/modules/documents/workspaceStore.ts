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
 *
 * Recovery drafts (stage 2, ADR 0016): at most `DRAFT_DELAY_MS` after the
 * first unsaved input of a tab its text goes to Rust (`recovery_write`),
 * also while the user keeps typing; a saved or discarded tab drops its
 * draft.
 */
import { create } from 'zustand';
import { ask } from '@/components/prompt/promptStore';
import { translate, useLocale, type MessageKey } from '@/i18n';
import { convertContent, needsConfirmation, type Conversion } from '@/modules/editor/convert';
import { LOSS_KEYS } from '@/modules/editor/markdown/lossKeys';
import type { Loss } from '@/modules/editor/markdown/losses';
import type { ProbeIssue } from '@/modules/editor/markdown/probe';
import { closeAllSessions, closeSession, getSession, type SaveRequest } from '@/modules/editor/session';
import { useMedia } from '@/modules/media/mediaStore';
import { useUi } from '@/modules/settings/uiStore';
import { setDocumentReloader, useStructure } from '@/modules/structure/structureStore';
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
/** A draft is written at most this long after the first unsaved input. */
export const DRAFT_DELAY_MS = 1000;

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
  /** The user closed the question for now; a note in the tab reopens it. */
  outsideDeferred: boolean;
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

/**
 * A tab that shows no document: the media gallery or one medium (Phase 8,
 * ADR 0019). Its id starts with `view:`; it has nothing to save.
 */
export type ViewTab = { kind: 'gallery' } | { kind: 'media'; mediaId: string; page: number | null };

/** Id of the gallery tab. */
export const GALLERY_TAB = 'view:gallery';

/** Id of the tab of a medium. */
export function mediaTabId(mediaId: string): string {
  return `view:media:${mediaId}`;
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
  /** Tabs of the gallery and of media. */
  views: Record<string, ViewTab>;
  /** Opens (or shows) the media gallery. */
  openGallery: () => void;
  /** Opens (or shows) a medium; a PDF at `page`. */
  openMedia: (mediaId: string, page?: number | null) => void;
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
  /**
   * Opens a file Rust registered (dialog, recent list, start argument). With
   * `draftText` the tab shows that text as an unsaved change (stage 2).
   */
  openExternal: (info: ExternalFileInfo, draftText?: string) => Promise<boolean>;
  openRecentFile: (recentId: string, name: string) => Promise<boolean>;
  forgetRecentFile: (recentId: string) => Promise<void>;
  refreshRecentFiles: () => Promise<void>;
  /** "Speichern unter" for an external tab. */
  saveAs: (id: string, dialogTitle: string) => Promise<boolean>;
  /** "Neu laden" after a change from outside: the file's text replaces the tab's. */
  reloadExternal: (id: string) => Promise<boolean>;
  /** "Meine Fassung behalten": the next save writes the tab's text over the file. */
  keepMine: (id: string) => Promise<boolean>;
  /** Hides or shows again the question about a change from outside. */
  deferOutside: (id: string, deferred: boolean) => void;
  setEncoding: (id: string, encoding: TextEncoding) => void;
  setLineEnding: (id: string, lineEnding: LineEnding) => void;
  /** Asks Rust whether external files changed outside the app. */
  checkExternalFiles: () => Promise<void>;
  /** "In Bibliothek aufnehmen": the tab then shows the library document. */
  takeIntoLibrary: (id: string) => Promise<string | null>;
  /** Before quitting: asks for each external tab with changes. False: stay. */
  confirmUnsaved: () => Promise<boolean>;
  /** Converts a library document to another format, after a warning about losses. */
  convertDocument: (id: string, format: TextFormat) => Promise<boolean>;
  /**
   * Opens the files passed at start or by a second start (ADR 0015,
   * decision 15), once the workbench has loaded the library.
   */
  takeOpenRequests: () => Promise<void>;
  /** Documents in the trash (stage 3), most recently deleted first. */
  trash: DocumentSummary[];
  refreshTrash: () => Promise<void>;
  /** Moves a document to the trash; its tab closes after saving. */
  deleteDocument: (id: string) => Promise<boolean>;
  /** Takes a document out of the trash (stage 3). */
  restoreDocument: (id: string) => Promise<boolean>;
  archiveDocument: (id: string, archived: boolean) => Promise<boolean>;
  /** "Endgültig löschen", after a question. */
  purgeDocument: (id: string) => Promise<boolean>;
  /** "Papierkorb leeren", after a question. */
  emptyTrash: () => Promise<boolean>;
  /** Restores a version (stage 4) after saving the tab; the tab reloads. */
  restoreVersion: (id: string, versionId: string) => Promise<boolean>;
  /** Reads a library document again and starts its editor anew. */
  reloadLibraryDocument: (id: string) => Promise<boolean>;
  /** "Version speichern" (saved first); the error, or null. */
  saveVersion: (id: string, name: string) => Promise<CommandError | null>;
  /** Versions "beim Schließen" of every open library tab (before quitting). */
  recordCloseVersions: () => Promise<void>;
}

const timers = new Map<string, ReturnType<typeof setTimeout>>();
const saving = new Map<string, Promise<boolean>>();
const sources = new Map<string, DocumentSource>();
/** Counts saves of external tabs, so a check that crossed a save is ignored. */
const saveEpochs = new Map<string, number>();
const draftTimers = new Map<string, ReturnType<typeof setTimeout>>();
/** Tabs that may have a draft in Rust (written, or restored from one). */
const drafted = new Set<string>();

function draftTarget(document: OpenDocument) {
  return document.external
    ? ({ kind: 'external', id: document.external.fileId } as const)
    : ({ kind: 'library', id: document.id } as const);
}

/** The workbench has loaded the library: files from outside can open. */
let openRequestsReady = false;

export function setOpenRequestsReady(ready: boolean): void {
  openRequestsReady = ready;
}

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
    outsideDeferred: false,
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
    outsideDeferred: false,
    differsFromFile: false,
  };
}

const FORMAT_KEYS: Record<DocumentFormat, MessageKey> = {
  txt: 'format.txt',
  md: 'format.md',
  pwrite: 'format.pwrite',
};

/** What a conversion loses, as one line of text. */
export function conversionDetails(conversion: Conversion): string {
  const text = (key: MessageKey) => translate(useLocale.getState().locale, key);
  const parts = [...new Set(conversion.losses.map((loss) => text(LOSS_KEYS[loss.kind])))];
  if (conversion.formattingLost) {
    parts.push(text('convert.formattingLost'));
  }
  if (conversion.frontMatterLost) {
    parts.push(text('convert.frontMatterLost'));
  }
  if (conversion.issues.length > 0) {
    parts.push(text('convert.issues'));
  }
  return parts.join('; ');
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
      if (unchangedSince) {
        discardDraft(id);
      } else {
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
        patchExternal(id, {
          outside: code === 'FILE_CHANGED' ? 'changed' : 'missing',
          outsideDeferred: false,
        });
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
      if (unchangedSince) {
        discardDraft(id);
      } else if (autosaveExternal()) {
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

  /** Writes the draft of a tab now, if it still has unsaved changes. */
  const writeDraft = (id: string) => {
    draftTimers.delete(id);
    const document = get().open[id];
    if (!document?.loaded || document.readOnly) {
      return;
    }
    const text = getSession(id)?.text();
    if (text === undefined || !hasUnsavedChanges(document)) {
      return;
    }
    drafted.add(id);
    const external = document.external;
    void call('recovery_write', {
      target: draftTarget(document),
      text,
      ...(external ? { encoding: external.encoding, lineEnding: external.lineEnding } : {}),
    });
  };

  /** Throttled, not debounced: continuous typing still writes every second. */
  const scheduleDraft = (id: string) => {
    if (!draftTimers.has(id)) {
      draftTimers.set(
        id,
        setTimeout(() => {
          writeDraft(id);
        }, DRAFT_DELAY_MS),
      );
    }
  };

  /** The tab is saved or its changes are gone: so is its draft. */
  const discardDraft = (id: string, document: OpenDocument | undefined = get().open[id]) => {
    const timer = draftTimers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      draftTimers.delete(id);
    }
    if (!document || !drafted.delete(id)) {
      return;
    }
    void call('recovery_discard', { target: draftTarget(document) });
  };

  const closeExternalFile = (fileId: string) => {
    void call('external_close', { id: fileId });
  };

  const removeTab = (id: string) => {
    clearTimer(id);
    // Closing follows saving or discarding: the draft is not needed.
    discardDraft(id);
    sources.delete(id);
    saveEpochs.delete(id);
    closeSession(id);
    set((state) => {
      const index = state.tabs.indexOf(id);
      const tabs = state.tabs.filter((tab) => tab !== id);
      const open = Object.fromEntries(Object.entries(state.open).filter(([key]) => key !== id));
      const views = Object.fromEntries(Object.entries(state.views).filter(([key]) => key !== id));
      let active = state.active;
      if (active === id) {
        active = tabs[Math.min(index, tabs.length - 1)] ?? null;
      }
      return { tabs, open, views, active, renaming: state.renaming === id ? null : state.renaming };
    });
  };

  return {
    documents: [],
    listLoaded: false,
    listError: null,
    tabs: [],
    active: null,
    views: {},
    open: {},
    renaming: null,
    lastError: null,
    recentFiles: [],
    trash: [],

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
      scheduleDraft(id);
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

    openGallery: () => {
      const previous = get().active;
      set((state) => ({
        tabs: state.tabs.includes(GALLERY_TAB) ? state.tabs : [...state.tabs, GALLERY_TAB],
        views: { ...state.views, [GALLERY_TAB]: { kind: 'gallery' } },
        active: GALLERY_TAB,
      }));
      if (previous && previous !== GALLERY_TAB) {
        void get().flush(previous);
      }
    },

    openMedia: (mediaId, page = null) => {
      const id = mediaTabId(mediaId);
      const previous = get().active;
      set((state) => ({
        tabs: state.tabs.includes(id) ? state.tabs : [...state.tabs, id],
        views: { ...state.views, [id]: { kind: 'media', mediaId, page } },
        active: id,
      }));
      if (previous && previous !== id) {
        void get().flush(previous);
      }
    },

    activate: (id) => {
      const previous = get().active;
      if (previous === id || (!get().open[id] && !get().views[id])) {
        return;
      }
      set({ active: id });
      if (previous) {
        void get().flush(previous);
      }
    },

    closeTab: async (id) => {
      if (get().views[id]) {
        removeTab(id);
        return true;
      }
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
      // Stage 4: a version "beim Schließen" (Rust skips unchanged content).
      if (document?.loaded && !document.readOnly) {
        void call('document_version_close', { id });
      }
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
      // The copy has the subnets and tags of the original (section 13).
      void useStructure.getState().refreshDocument(result.value.id);
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
          const timer = draftTimers.get(id);
          if (timer !== undefined) {
            clearTimeout(timer);
            draftTimers.delete(id);
          }
          drafted.delete(id);
          sources.delete(id);
          closeSession(id);
        }
      }
      if (kept.length === 0) {
        closeAllSessions();
      }
      set({
        documents: [],
        trash: [],
        listLoaded: false,
        listError: null,
        tabs: kept,
        active: active !== null && kept.includes(active) ? active : (kept[0] ?? null),
        open: Object.fromEntries(kept.flatMap((id) => (open[id] ? [[id, open[id]]] : []))),
        // Gallery and media belong to the library.
        views: {},
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

    openExternal: async (info, draftText) => {
      const tab = externalTabId(info.id);
      const existing = get().open[tab];
      if (existing) {
        get().activate(tab);
        if (draftText !== undefined && existing.loaded) {
          // The draft replaces what the tab shows; the file stays as it is.
          sources.delete(tab);
          closeSession(tab);
          patch(tab, {
            loadedText: draftText,
            state: 'pending',
            revision: existing.revision + 1,
            generation: existing.generation + 1,
          });
          drafted.add(tab);
        }
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
      const restored = draftText !== undefined && !text.newerFormat;
      patch(tab, {
        loadedText: restored ? draftText : text.text,
        savedText: text.text,
        loaded: true,
        readOnly: text.newerFormat,
        ...(restored ? { state: 'pending' as const } : {}),
      });
      patchExternal(tab, loadedExternal(text));
      if (restored) {
        drafted.add(tab);
      }
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
        outsideDeferred: false,
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
      discardDraft(id);
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
      patchExternal(id, { outside: null, outsideDeferred: false, differsFromFile: true });
      touchExternal(id);
      return true;
    },

    deferOutside: (id, deferred) => {
      patchExternal(id, { outsideDeferred: deferred });
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
          // A .pwrite brings the tags of its envelope.
          void useStructure.getState().refreshDocument(target.id);
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

    convertDocument: async (id, format) => {
      const document = get().open[id];
      const session = getSession(id);
      if (
        !document?.loaded ||
        document.external ||
        document.readOnly ||
        !session ||
        document.format === format
      ) {
        return false;
      }
      const conversion = convertContent(document.format, format, session.conversionSource());
      if (needsConfirmation(conversion)) {
        const answer = await ask({
          titleKey: 'convert.title',
          textKey: 'convert.warning',
          params: {
            format: translate(useLocale.getState().locale, FORMAT_KEYS[format]),
            details: conversionDetails(conversion),
          },
          choices: [
            { id: 'convert', labelKey: 'convert.anyway', tone: 'primary' },
            { id: 'cancel', labelKey: 'convert.cancel' },
          ],
          cancel: 'cancel',
          testId: 'convert-prompt',
        });
        if (answer !== 'convert') {
          return false;
        }
      }
      // The conversion takes the editor's text; a pending save is not needed.
      clearTimer(id);
      await saving.get(id);
      const result = await call('document_convert', { id, format, text: conversion.text });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      sources.delete(id);
      closeSession(id);
      const current = get().open[id] ?? document;
      patch(id, {
        format,
        loadedText: conversion.text,
        savedText: conversion.text,
        state: 'saved',
        error: null,
        losses: [],
        viewIssues: [],
        view: format === 'txt' ? 'plain' : 'rich',
        revision: current.revision + 1,
        generation: current.generation + 1,
      });
      set((state) => ({ documents: upsert(state.documents, result.value) }));
      return true;
    },

    refreshTrash: async () => {
      const result = await call('documents_trash', {});
      if (result.ok) {
        set({ trash: result.value });
      }
    },

    deleteDocument: async (id) => {
      const document = get().open[id];
      if (document) {
        // What is open is saved first; a failed save keeps the tab.
        if (!(await get().flush(id))) {
          return false;
        }
        removeTab(id);
      }
      const result = await call('document_delete', { id });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      const deleted = result.value;
      set((state) => ({
        documents: state.documents.filter((entry) => entry.id !== id),
        trash: [deleted, ...state.trash.filter((entry) => entry.id !== id)],
      }));
      // A step of the structural undo (ADR 0017, decision 10).
      void useStructure.getState().noteStep();
      return true;
    },

    restoreDocument: async (id) => {
      const result = await call('document_restore', { id });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      const restored = result.value;
      set((state) => ({
        documents: upsert(state.documents, restored),
        trash: state.trash.filter((entry) => entry.id !== id),
      }));
      void useStructure.getState().noteStep();
      return true;
    },

    archiveDocument: async (id, archived) => {
      const result = await call('document_archive', { id, archived });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set((state) => ({ documents: upsert(state.documents, result.value) }));
      void useStructure.getState().noteStep();
      return true;
    },

    purgeDocument: async (id) => {
      const entry = get().trash.find((document) => document.id === id);
      const answer = await ask({
        titleKey: 'trash.purge.title',
        textKey: 'trash.purge.text',
        params: { title: entry?.title ?? '' },
        choices: [
          { id: 'purge', labelKey: 'trash.purge.confirm', tone: 'danger' },
          { id: 'cancel', labelKey: 'trash.cancel' },
        ],
        cancel: 'cancel',
        testId: 'purge-prompt',
      });
      if (answer !== 'purge') {
        return false;
      }
      const result = await call('document_purge', { id });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set((state) => ({ trash: state.trash.filter((document) => document.id !== id) }));
      // Its subnets and tags went with it.
      useStructure.getState().forget(id);
      return true;
    },

    emptyTrash: async () => {
      // Documents and media share the trash of the Explorer.
      const count = get().trash.length + useMedia.getState().trash.length;
      if (count === 0) {
        return false;
      }
      const answer = await ask({
        titleKey: 'trash.empty.title',
        textKey: 'trash.empty.text',
        params: { count },
        choices: [
          { id: 'empty', labelKey: 'trash.empty.confirm', tone: 'danger' },
          { id: 'cancel', labelKey: 'trash.cancel' },
        ],
        cancel: 'cancel',
        testId: 'empty-trash-prompt',
      });
      if (answer !== 'empty') {
        return false;
      }
      const result = await call('trash_empty', {});
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set({ trash: [] });
      // Media a document still uses stay (section 9).
      await useMedia.getState().emptyTrash();
      void useStructure.getState().load();
      return true;
    },

    restoreVersion: async (id, versionId) => {
      if (get().open[id] && !(await get().flush(id))) {
        return false;
      }
      const result = await call('document_version_restore', { id: versionId });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      set((state) => ({ documents: upsert(state.documents, result.value) }));
      if (get().open[id]) {
        await get().reloadLibraryDocument(id);
      }
      useUi.getState().showNotice({ tone: 'info', key: 'history.restored' });
      return true;
    },

    reloadLibraryDocument: async (id) => {
      const result = await call('document_read', { id });
      if (!result.ok) {
        set({ lastError: result.error });
        return false;
      }
      const { document, text, readOnly } = result.value;
      clearTimer(id);
      sources.delete(id);
      closeSession(id);
      const current = get().open[id];
      if (!current) {
        return true;
      }
      patch(id, {
        title: document.title,
        format: document.format,
        loadedText: text,
        savedText: text,
        state: 'saved',
        error: null,
        losses: [],
        viewIssues: [],
        view: document.format === 'txt' ? 'plain' : 'rich',
        readOnly,
        revision: current.revision + 1,
        generation: current.generation + 1,
      });
      set((state) => ({ documents: upsert(state.documents, document) }));
      return true;
    },

    saveVersion: async (id, name) => {
      if (get().open[id] && !(await get().flush(id))) {
        return get().open[id]?.error ?? null;
      }
      const result = await call('document_version_save', { id, name });
      if (!result.ok) {
        return result.error;
      }
      useUi.getState().showNotice({ tone: 'info', key: 'history.saved', params: { name: name.trim() } });
      return null;
    },

    recordCloseVersions: async () => {
      const library = get().tabs.filter((id) => {
        const document = get().open[id];
        return document?.loaded === true && !document.external && !document.readOnly;
      });
      await Promise.all(library.map((id) => call('document_version_close', { id })));
    },

    takeOpenRequests: async () => {
      if (!openRequestsReady) {
        return;
      }
      const result = await call('open_requests_take', {});
      if (!result.ok) {
        set({ lastError: result.error });
        return;
      }
      for (const target of result.value) {
        if (target.kind === 'library') {
          await get().openDocument(target.id);
        } else if (target.kind === 'external') {
          await get().openExternal(target.file);
        } else {
          set({ lastError: target.error });
        }
      }
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

// Undoing a trash or archive step reloads the document lists.
setDocumentReloader(async () => {
  await useWorkspace.getState().refresh();
  await useWorkspace.getState().refreshTrash();
});
