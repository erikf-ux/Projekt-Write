/**
 * Media in the interface (Phase 8, ADR 0019): the list, the trash, details,
 * favourites, archive, the documents that use a medium, thumbnails of PDFs
 * and the import dialog.
 *
 * Rust holds the data; trash and archive are steps of the structural undo
 * (the structure store shows the notice with "Rückgängig").
 */
import { create } from 'zustand';
import { ask } from '@/components/prompt/promptStore';
import { useUi } from '@/modules/settings/uiStore';
import { setMediaLookup, setMediaReloader, useStructure } from '@/modules/structure/structureStore';
import { call, callWithProgress, importMediaBytes } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { Decision } from '@/types/generated/Decision';
import type { ImportOutcome } from '@/types/generated/ImportOutcome';
import type { ImportPreview } from '@/types/generated/ImportPreview';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import type { MediaUse } from '@/types/generated/MediaUse';

/** Where an import comes from. */
export type ImportSource = 'files' | 'folder' | 'drop';

export type ImportState =
  | { phase: 'preparing'; batchId: string; source: ImportSource; done: number; total: number }
  | { phase: 'review'; preview: ImportPreview; source: ImportSource; committing: boolean }
  | null;

/** What the review dialog decided. */
export interface ImportChoice {
  decisions: Record<number, Decision>;
  duplicates: Decision;
  subnetId: string | null;
  fromFolders: boolean;
}

interface MediaStore {
  loaded: boolean;
  media: MediaSummary[];
  trash: MediaSummary[];
  uses: Record<string, MediaUse[]>;
  /** Changes when a thumbnail was stored, so the gallery loads it again. */
  thumbnailVersion: Record<string, number>;
  importing: ImportState;
  /** Resolves the import dialog of the current batch. */
  lastImport: ImportOutcome | null;
  lastError: CommandError | null;
  load: () => Promise<void>;
  refresh: () => Promise<void>;
  refreshTrash: () => Promise<void>;
  reset: () => void;
  upsert: (summary: MediaSummary) => void;
  update: (id: string, change: { title?: string; description?: string }) => Promise<boolean>;
  setFavorite: (id: string, favorite: boolean) => Promise<boolean>;
  markOpened: (id: string) => Promise<void>;
  archive: (id: string, archived: boolean) => Promise<boolean>;
  remove: (id: string) => Promise<boolean>;
  restore: (id: string) => Promise<boolean>;
  purge: (id: string) => Promise<boolean>;
  /** Empties the media part of the trash (after the documents' prompt). */
  emptyTrash: () => Promise<void>;
  loadUses: (id: string) => Promise<void>;
  openExternal: (id: string) => Promise<boolean>;
  storeThumbnail: (id: string, pngBase64: string) => Promise<void>;
  setPageCount: (id: string, pages: number) => Promise<void>;
  /** Shows the system dialog and prepares the chosen files or folder. */
  startImport: (source: 'files' | 'folder', dialogTitle: string) => Promise<void>;
  /** Prepares files dropped onto the window or pasted; with no duplicate they are imported at once. */
  importFiles: (files: readonly File[], subnetId?: string | null) => Promise<ImportOutcome | null>;
  commitImport: (choice: ImportChoice) => Promise<ImportOutcome | null>;
  cancelImport: () => Promise<void>;
}

function upsertInto(list: readonly MediaSummary[], summary: MediaSummary): MediaSummary[] {
  const index = list.findIndex((entry) => entry.id === summary.id);
  if (index === -1) {
    return [summary, ...list];
  }
  const next = [...list];
  next[index] = summary;
  return next;
}

function showError(error: CommandError) {
  useUi.getState().showNotice({ tone: 'error', key: 'media.failed', error });
}

function newBatchId(): string {
  return crypto.randomUUID();
}

/** Dropped files waiting while the dialog asks about the one before them. */
let queuedDrop: { files: readonly File[]; subnetId: string | null } | null = null;

/** Goes on with the dropped files after the dialog of one was closed. */
function continueDrop(store: () => MediaStore) {
  const next = queuedDrop;
  queuedDrop = null;
  if (next && next.files.length > 0) {
    void store().importFiles(next.files, next.subnetId);
  }
}

/** Bytes of a file for the import (Rust checks type and size). */
async function bytesOf(file: File): Promise<Uint8Array> {
  return new Uint8Array(await file.arrayBuffer());
}

export const useMedia = create<MediaStore>((set, get) => ({
  loaded: false,
  media: [],
  trash: [],
  uses: {},
  thumbnailVersion: {},
  importing: null,
  lastImport: null,
  lastError: null,

  load: async () => {
    await Promise.all([get().refresh(), get().refreshTrash()]);
    set({ loaded: true });
  },

  refresh: async () => {
    const result = await call('media_list', {});
    if (result.ok) {
      set({ media: result.value });
    } else {
      set({ lastError: result.error });
    }
  },

  refreshTrash: async () => {
    const result = await call('media_trash_list', {});
    if (result.ok) {
      set({ trash: result.value });
    }
  },

  reset: () => {
    queuedDrop = null;
    set({
      loaded: false,
      media: [],
      trash: [],
      uses: {},
      thumbnailVersion: {},
      importing: null,
      lastImport: null,
      lastError: null,
    });
  },

  upsert: (summary) => {
    set((state) => ({ media: upsertInto(state.media, summary) }));
  },

  update: async (id, change) => {
    const result = await call('media_update', { id, ...change });
    if (!result.ok) {
      set({ lastError: result.error });
      showError(result.error);
      return false;
    }
    get().upsert(result.value);
    return true;
  },

  setFavorite: async (id, favorite) => {
    const result = await call('media_set_favorite', { id, on: favorite });
    if (!result.ok) {
      showError(result.error);
      return false;
    }
    get().upsert(result.value);
    return true;
  },

  markOpened: async (id) => {
    const result = await call('media_mark_opened', { id });
    if (result.ok) {
      get().upsert(result.value);
    }
  },

  archive: async (id, archived) => {
    const result = await call('media_archive', { id, on: archived });
    if (!result.ok) {
      showError(result.error);
      return false;
    }
    get().upsert(result.value);
    void useStructure.getState().noteStep();
    return true;
  },

  remove: async (id) => {
    const result = await call('media_delete', { id });
    if (!result.ok) {
      showError(result.error);
      return false;
    }
    const deleted = result.value;
    set((state) => ({
      media: state.media.filter((entry) => entry.id !== id),
      trash: [deleted, ...state.trash.filter((entry) => entry.id !== id)],
    }));
    void useStructure.getState().noteStep();
    return true;
  },

  restore: async (id) => {
    const result = await call('media_restore', { id });
    if (!result.ok) {
      showError(result.error);
      return false;
    }
    set((state) => ({
      media: upsertInto(state.media, result.value),
      trash: state.trash.filter((entry) => entry.id !== id),
    }));
    void useStructure.getState().noteStep();
    return true;
  },

  purge: async (id) => {
    const entry = get().trash.find((medium) => medium.id === id);
    const answer = await ask({
      titleKey: 'media.purge.title',
      textKey: 'media.purge.text',
      params: { title: entry?.title ?? '' },
      choices: [
        { id: 'purge', labelKey: 'media.purge.confirm', tone: 'danger' },
        { id: 'cancel', labelKey: 'media.cancel' },
      ],
      cancel: 'cancel',
      testId: 'media-purge-prompt',
    });
    if (answer !== 'purge') {
      return false;
    }
    const result = await call('media_purge', { id });
    if (!result.ok) {
      if (result.error.code === 'MEDIA_IN_USE') {
        useUi.getState().showNotice({
          tone: 'info',
          key: 'media.purge.inUse',
          params: { title: entry?.title ?? '', count: Number(result.error.details?.['documents'] ?? 1) },
        });
      } else {
        showError(result.error);
      }
      return false;
    }
    set((state) => ({ trash: state.trash.filter((medium) => medium.id !== id) }));
    useStructure.getState().forget(id);
    return true;
  },

  emptyTrash: async () => {
    if (get().trash.length === 0) {
      return;
    }
    const result = await call('media_empty_trash', {});
    if (!result.ok) {
      showError(result.error);
      return;
    }
    await get().refreshTrash();
    if (result.value.keptInUse > 0) {
      useUi.getState().showNotice({
        tone: 'info',
        key: 'media.emptyTrash.kept',
        params: { count: result.value.keptInUse },
      });
    }
  },

  loadUses: async (id) => {
    const result = await call('media_uses', { id });
    if (result.ok) {
      set((state) => ({ uses: { ...state.uses, [id]: result.value } }));
    }
  },

  openExternal: async (id) => {
    const result = await call('media_open_external', { id });
    if (!result.ok) {
      showError(result.error);
      return false;
    }
    useUi.getState().showNotice({ tone: 'info', key: 'media.openedAsCopy' });
    return true;
  },

  storeThumbnail: async (id, pngBase64) => {
    const result = await call('media_store_thumbnail', { id, pngBase64 });
    if (result.ok) {
      set((state) => ({
        thumbnailVersion: { ...state.thumbnailVersion, [id]: (state.thumbnailVersion[id] ?? 0) + 1 },
      }));
    }
  },

  setPageCount: async (id, pages) => {
    const medium = get().media.find((entry) => entry.id === id);
    if (medium?.pageCount === pages) {
      return;
    }
    const result = await call('media_set_page_count', { id, pages });
    if (result.ok && medium) {
      get().upsert({ ...medium, pageCount: pages });
    }
  },

  startImport: async (source, dialogTitle) => {
    if (get().importing) {
      return;
    }
    const batchId = newBatchId();
    set({ importing: { phase: 'preparing', batchId, source, done: 0, total: 0 } });
    const result = await callWithProgress(
      source === 'files' ? 'media_import_files' : 'media_import_folder',
      { batchId, title: dialogTitle },
      (progress) => {
        set((state) =>
          state.importing?.phase === 'preparing' && state.importing.batchId === progress.batchId
            ? { importing: { ...state.importing, done: progress.done, total: progress.total } }
            : {},
        );
      },
    );
    if (!result.ok) {
      set({ importing: null });
      // A cancelled preparation is no error.
      if (result.error.details?.['cancelled'] !== 'true') {
        showError(result.error);
      }
      return;
    }
    if (!result.value) {
      set({ importing: null });
      return;
    }
    set({ importing: { phase: 'review', preview: result.value, source, committing: false } });
  },

  importFiles: async (files, subnetId = null) => {
    if (get().importing) {
      return null;
    }
    const items: ImportOutcome['items'] = [];
    let asking = false;
    for (const file of files) {
      const prepared = await importMediaBytes(file.name, await bytesOf(file));
      if (!prepared.ok) {
        showError(prepared.error);
        continue;
      }
      const preview = prepared.value;
      if (preview.items.length === 0 || preview.items.some((item) => item.duplicate !== null)) {
        // Refused (the dialog names the reason) or a duplicate, which is
        // asked about (section 9). The files after it wait until the
        // dialog is closed, either way.
        queuedDrop = { files: files.slice(files.indexOf(file) + 1), subnetId };
        set({ importing: { phase: 'review', preview, source: 'drop', committing: false } });
        asking = true;
        break;
      }
      const committed = await call('media_import_commit', {
        batchId: preview.batchId,
        duplicates: 'useExisting',
        ...(subnetId ? { subnetId } : {}),
      });
      if (!committed.ok) {
        showError(committed.error);
        continue;
      }
      items.push(...committed.value.items);
    }
    if (items.length === 0) {
      return null;
    }
    await Promise.all([get().refresh(), useStructure.getState().load()]);
    const outcome: ImportOutcome = { items, subnetId: null };
    if (!asking) {
      set({ lastImport: outcome });
    }
    return outcome;
  },

  commitImport: async (choice) => {
    const state = get().importing;
    if (state?.phase !== 'review') {
      return null;
    }
    set({ importing: { ...state, committing: true } });
    const decisions = Object.entries(choice.decisions).map(([index, decision]) => ({
      index: Number(index),
      decision,
    }));
    const result = await call('media_import_commit', {
      batchId: state.preview.batchId,
      decisions,
      duplicates: choice.duplicates,
      ...(choice.subnetId ? { subnetId: choice.subnetId } : {}),
      fromFolders: choice.fromFolders,
    });
    set({ importing: null });
    if (!result.ok) {
      showError(result.error);
      continueDrop(get);
      return null;
    }
    set({ lastImport: result.value });
    await Promise.all([get().refresh(), useStructure.getState().load()]);
    const created = result.value.items.filter((item) => item.created).length;
    useUi.getState().showNotice({
      tone: 'info',
      key: 'media.import.done',
      params: { count: result.value.items.length, created },
    });
    continueDrop(get);
    return result.value;
  },

  cancelImport: async () => {
    const state = get().importing;
    set({ importing: null });
    if (state?.phase === 'preparing') {
      await call('media_import_cancel', { batchId: state.batchId });
    } else if (state?.phase === 'review') {
      await call('media_import_cancel', { batchId: state.preview.batchId });
    }
    // Closing the dialog of one dropped file leaves out that file only.
    continueDrop(get);
  },
}));

/** A medium by id: in the list or the trash. */
export function mediaById(state: Pick<MediaStore, 'media' | 'trash'>, id: string): MediaSummary | undefined {
  return state.media.find((entry) => entry.id === id) ?? state.trash.find((entry) => entry.id === id);
}

// The structure commands need the kind of an element; undoing a trash or
// archive step of a medium reloads the media lists.
setMediaLookup((id) => mediaById(useMedia.getState(), id) !== undefined);
setMediaReloader(async () => {
  await Promise.all([useMedia.getState().refresh(), useMedia.getState().refreshTrash()]);
});
