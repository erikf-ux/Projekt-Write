import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePrompt } from '@/components/prompt/promptStore';
import { useUi } from '@/modules/settings/uiStore';
import type { UiState } from '@/types/generated/UiState';
import {
  AUTOSAVE_DELAY_MS,
  externalTabId,
  hasUnsavedChanges,
  setDocumentSource,
  useWorkspace,
} from './workspaceStore';

const FILE = '0123456789abcdef0123456789abcdef';
const TAB = externalTabId(FILE);
const LIBRARY_ID = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

interface SaveCall {
  text: string;
  encoding: string;
  lineEnding: string;
  overwrite: boolean;
}

interface Backend {
  text: string;
  encoding: string;
  lineEnding: string;
  size: number;
  changedOutside: boolean;
  saveError: { code: string; messageKey: string } | null;
  saves: SaveCall[];
  closed: string[];
  reads: number;
}

function info(backend: Backend, name = 'notiz.txt') {
  return {
    id: FILE,
    name,
    folder: '/home/a/Texte',
    format: 'txt',
    size: backend.size,
    large: backend.size >= 10 * 1024 * 1024,
  };
}

function summary(id: string, title: string) {
  return {
    id,
    title,
    format: 'txt',
    sizeBytes: 0,
    createdAt: '2026-11-04T10:00:00.000Z',
    updatedAt: '2026-11-04T10:00:00.000Z',
    originalName: 'notiz.txt',
    favoritedAt: null,
    lastOpenedAt: '2026-11-04T10:00:00.000Z',
  };
}

function mockBackend(): Backend {
  const backend: Backend = {
    text: 'Größe\n',
    encoding: 'windows1252',
    lineEnding: 'crlf',
    size: 7,
    changedOutside: false,
    saveError: null,
    saves: [],
    closed: [],
    reads: 0,
  };
  mockIPC((command, args) => {
    const request = (args as { request: Record<string, unknown> }).request;
    switch (command) {
      case 'external_open':
        return info(backend);
      case 'external_read':
        backend.reads += 1;
        backend.changedOutside = false;
        return {
          text: backend.text,
          encoding: backend.encoding,
          lineEnding: backend.lineEnding,
          mixedLineEndings: false,
          readOnly: false,
          newerFormat: false,
        };
      case 'external_check':
        return { status: backend.changedOutside ? 'changed' : 'unchanged', readOnly: false };
      case 'external_keep_mine':
        backend.changedOutside = false;
        return null;
      case 'external_save': {
        if (backend.saveError) {
          throw backend.saveError;
        }
        if (backend.changedOutside && request['overwrite'] !== true) {
          throw { code: 'FILE_CHANGED', messageKey: 'error.fileChanged' };
        }
        backend.saves.push(request as unknown as SaveCall);
        return 'saved';
      }
      case 'external_save_as':
        backend.saves.push(request as unknown as SaveCall);
        return info(backend, 'kopie.txt');
      case 'external_close':
        backend.closed.push(String(request['id']));
        return null;
      case 'external_recent':
        return [];
      case 'external_import':
        return { kind: 'imported', document: summary(LIBRARY_ID, 'notiz') };
      case 'document_read':
        return { document: summary(LIBRARY_ID, 'notiz'), text: backend.text, readOnly: false };
      default:
        throw new Error(`unexpected ${command}`);
    }
  });
  return backend;
}

let editorText = '';

/** The open document of the external tab. */
function tab() {
  const document = useWorkspace.getState().open[TAB];
  if (!document) {
    throw new Error('external tab not open');
  }
  return document;
}

/** The editor session of the tab: holds the text. */
function attachEditor(id: string, text: string): void {
  editorText = text;
  setDocumentSource(id, {
    saveRequest: () => ({ kind: 'text', text: editorText }),
    acceptLosses: () => undefined,
  });
}

function type(id: string, text: string): void {
  editorText = text;
  useWorkspace.getState().edit(id);
}

async function answerPrompt(answer: string): Promise<void> {
  await vi.waitFor(() => {
    expect(usePrompt.getState().queue.length).toBeGreaterThan(0);
  });
  usePrompt.getState().answer(answer);
}

function setAutosave(on: boolean): void {
  useUi.setState({ state: { settings: { autosaveExternal: on } } as unknown as UiState });
}

describe('external files', () => {
  let backend: Backend;

  beforeEach(async () => {
    vi.useFakeTimers();
    backend = mockBackend();
    setAutosave(false);
    expect(await useWorkspace.getState().openFile('Datei öffnen')).toBe(true);
    attachEditor(TAB, backend.text);
  });

  afterEach(() => {
    vi.useRealTimers();
    clearMocks();
    usePrompt.setState({ queue: [] });
    useWorkspace.setState({ tabs: [], open: {}, active: null });
    useUi.setState({ state: null });
  });

  it('opens with the encoding and line endings of the file', () => {
    const document = useWorkspace.getState().open[TAB];
    expect(document?.title).toBe('notiz.txt');
    expect(document?.loadedText).toBe('Größe\n');
    expect(document?.external).toMatchObject({
      encoding: 'windows1252',
      lineEnding: 'crlf',
      folder: '/home/a/Texte',
    });
    expect(useWorkspace.getState().active).toBe(TAB);
  });

  it('saves only with Ctrl+S unless autosave is on, in the encoding of the file', async () => {
    type(TAB, 'Größe 2\n');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS * 3);
    expect(backend.saves).toEqual([]);
    // Focus loss or a tab switch does not save either.
    expect(await useWorkspace.getState().flush(TAB)).toBe(true);
    expect(backend.saves).toEqual([]);
    expect(hasUnsavedChanges(tab())).toBe(true);

    expect(await useWorkspace.getState().save(TAB)).toBe(true);
    expect(backend.saves).toEqual([
      { id: FILE, text: 'Größe 2\n', encoding: 'windows1252', lineEnding: 'crlf', overwrite: false },
    ]);
    expect(hasUnsavedChanges(tab())).toBe(false);
    // Unchanged: nothing is written.
    expect(await useWorkspace.getState().save(TAB)).toBe(true);
    expect(backend.saves).toHaveLength(1);

    setAutosave(true);
    type(TAB, 'Größe 3\n');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS);
    await vi.waitFor(() => {
      expect(backend.saves).toHaveLength(2);
    });
  });

  it('asks before closing a tab with changes', async () => {
    type(TAB, 'geändert\n');
    const cancelled = useWorkspace.getState().closeTab(TAB);
    await answerPrompt('cancel');
    expect(await cancelled).toBe(false);
    expect(useWorkspace.getState().tabs).toEqual([TAB]);

    const saved = useWorkspace.getState().closeTab(TAB);
    await answerPrompt('save');
    expect(await saved).toBe(true);
    expect(backend.saves.map((save) => save.text)).toEqual(['geändert\n']);
    expect(backend.closed).toEqual([FILE]);
    expect(useWorkspace.getState().tabs).toEqual([]);
  });

  it('discards on request and closes unchanged tabs without asking', async () => {
    type(TAB, 'verworfen\n');
    const closing = useWorkspace.getState().closeTab(TAB);
    await answerPrompt('discard');
    expect(await closing).toBe(true);
    expect(backend.saves).toEqual([]);
    expect(backend.closed).toEqual([FILE]);

    expect(await useWorkspace.getState().openFile('Datei öffnen')).toBe(true);
    attachEditor(TAB, backend.text);
    expect(await useWorkspace.getState().closeTab(TAB)).toBe(true);
    expect(usePrompt.getState().queue).toEqual([]);
  });

  it('reports a change from outside and writes my version only after keeping it', async () => {
    type(TAB, 'meine Fassung\n');
    backend.changedOutside = true;
    await useWorkspace.getState().checkExternalFiles();
    expect(useWorkspace.getState().open[TAB]?.external?.outside).toBe('changed');
    expect(await useWorkspace.getState().save(TAB)).toBe(false);
    expect(backend.saves).toEqual([]);

    expect(await useWorkspace.getState().keepMine(TAB)).toBe(true);
    expect(useWorkspace.getState().open[TAB]?.external?.outside).toBeNull();
    expect(await useWorkspace.getState().save(TAB)).toBe(true);
    expect(backend.saves.map((save) => save.text)).toEqual(['meine Fassung\n']);
  });

  it('keeping my version of an unchanged tab still writes it', async () => {
    backend.changedOutside = true;
    await useWorkspace.getState().checkExternalFiles();
    await useWorkspace.getState().keepMine(TAB);
    expect(hasUnsavedChanges(tab())).toBe(true);
    expect(await useWorkspace.getState().save(TAB)).toBe(true);
    expect(backend.saves.map((save) => save.text)).toEqual(['Größe\n']);
  });

  it('reloads the file and starts the editor anew', async () => {
    type(TAB, 'verworfen\n');
    backend.changedOutside = true;
    await useWorkspace.getState().checkExternalFiles();
    backend.text = 'von außen\n';
    expect(await useWorkspace.getState().reloadExternal(TAB)).toBe(true);
    const document = useWorkspace.getState().open[TAB];
    expect(document?.loadedText).toBe('von außen\n');
    expect(document?.generation).toBe(1);
    expect(document?.state).toBe('saved');
    expect(document?.external?.outside).toBeNull();
  });

  it('saves a copy and the tab follows it', async () => {
    type(TAB, 'Kopie\n');
    expect(await useWorkspace.getState().saveAs(TAB, 'Speichern unter')).toBe(true);
    const document = useWorkspace.getState().open[TAB];
    expect(document?.title).toBe('kopie.txt');
    expect(document?.state).toBe('saved');
    expect(backend.saves[0]).toMatchObject({ text: 'Kopie\n', encoding: 'windows1252' });
  });

  it('a new encoding or line ending makes the tab unsaved until written', async () => {
    useWorkspace.getState().setEncoding(TAB, 'utf8');
    expect(hasUnsavedChanges(tab())).toBe(true);
    useWorkspace.getState().setEncoding(TAB, 'windows1252');
    expect(hasUnsavedChanges(tab())).toBe(false);
    useWorkspace.getState().setLineEnding(TAB, 'lf');
    expect(await useWorkspace.getState().save(TAB)).toBe(true);
    expect(backend.saves).toEqual([
      { id: FILE, text: 'Größe\n', encoding: 'windows1252', lineEnding: 'lf', overwrite: false },
    ]);
  });

  it('keeps the change when the encoding cannot hold a character', async () => {
    backend.saveError = { code: 'FILE_UNENCODABLE', messageKey: 'error.fileUnencodable' };
    type(TAB, 'Pfeil →\n');
    expect(await useWorkspace.getState().save(TAB)).toBe(false);
    const document = useWorkspace.getState().open[TAB];
    expect(document?.state).toBe('error');
    expect(document?.error?.code).toBe('FILE_UNENCODABLE');
    expect(hasUnsavedChanges(tab())).toBe(true);
  });

  it('asks before opening a large file', async () => {
    await useWorkspace.getState().closeTab(TAB);
    backend.size = 12 * 1024 * 1024;
    const reads = backend.reads;
    const opening = useWorkspace.getState().openFile('Datei öffnen');
    await answerPrompt('cancel');
    expect(await opening).toBe(false);
    expect(backend.reads).toBe(reads);
    expect(useWorkspace.getState().tabs).toEqual([]);
  });

  it('takes the file into the library in the same tab', async () => {
    type(TAB, 'Größe\n');
    expect(await useWorkspace.getState().takeIntoLibrary(TAB)).toBe(LIBRARY_ID);
    expect(useWorkspace.getState().tabs).toEqual([LIBRARY_ID]);
    expect(useWorkspace.getState().open[LIBRARY_ID]?.external).toBeNull();
    expect(backend.closed).toEqual([FILE]);
  });

  it('asks for each external tab with changes before quitting', async () => {
    type(TAB, 'ungespeichert\n');
    const quitting = useWorkspace.getState().confirmUnsaved();
    await answerPrompt('cancel');
    expect(await quitting).toBe(false);
    const again = useWorkspace.getState().confirmUnsaved();
    await answerPrompt('discard');
    expect(await again).toBe(true);
  });
});
