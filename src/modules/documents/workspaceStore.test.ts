import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Loss } from '@/modules/editor/markdown/losses';
import { AUTOSAVE_DELAY_MS, freeTitle, setDocumentSource, useWorkspace } from './workspaceStore';

const ID_A = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';
const ID_B = '2f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

function summary(id: string, title: string) {
  return {
    id,
    title,
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-03T10:00:00.000Z',
    updatedAt: '2026-11-03T10:00:00.000Z',
    originalName: null,
  };
}

interface Backend {
  texts: Map<string, string>;
  saves: string[];
  failSave: boolean;
}

function mockBackend(): Backend {
  const backend: Backend = {
    texts: new Map([
      [ID_A, 'Alpha'],
      [ID_B, 'Beta'],
    ]),
    saves: [],
    failSave: false,
  };
  mockIPC((command, args) => {
    const request = (args as { request: Record<string, string> }).request;
    switch (command) {
      case 'documents_list':
        return [summary(ID_A, 'A'), summary(ID_B, 'B')];
      case 'document_read':
        return { document: summary(request['id'] ?? '', 'A'), text: backend.texts.get(request['id'] ?? '') };
      case 'document_save':
        if (backend.failSave) {
          throw { code: 'FILE_SYSTEM', messageKey: 'error.fileSystem' };
        }
        backend.saves.push(request['text'] ?? '');
        backend.texts.set(request['id'] ?? '', request['text'] ?? '');
        return summary(request['id'] ?? '', 'A');
      case 'document_create':
        return summary(ID_B, request['title'] ?? '');
      case 'document_rename':
        return summary(request['id'] ?? '', request['title'] ?? '');
      default:
        throw new Error(`unexpected ${command}`);
    }
  });
  return backend;
}

/** Stands in for the editor session of a tab: holds its text, may block saving. */
interface FakeSource {
  text: string;
  losses: Loss[];
  accepted: Loss[];
}

const fakeSources = new Map<string, FakeSource>();

function source(id: string, text: string): FakeSource {
  const fake: FakeSource = { text, losses: [], accepted: [] };
  fakeSources.set(id, fake);
  setDocumentSource(id, {
    saveRequest: () =>
      fake.losses.length > fake.accepted.length
        ? { kind: 'blocked', losses: fake.losses }
        : { kind: 'text', text: fake.text },
    acceptLosses: (losses) => {
      fake.accepted = [...losses];
    },
  });
  return fake;
}

/** Typing in the editor of a tab. */
function type(id: string, text: string): void {
  const fake = fakeSources.get(id) ?? source(id, '');
  fake.text = text;
  useWorkspace.getState().edit(id);
}

describe('workspace', () => {
  let backend: Backend;

  beforeEach(async () => {
    vi.useFakeTimers();
    backend = mockBackend();
    await useWorkspace.getState().reset();
    await useWorkspace.getState().refresh();
  });

  afterEach(() => {
    vi.useRealTimers();
    clearMocks();
    fakeSources.clear();
  });

  it('autosaves 1 s after the last input, once', async () => {
    const workspace = useWorkspace.getState();
    expect(await workspace.openDocument(ID_A)).toBe(true);
    expect(useWorkspace.getState().open[ID_A]?.loadedText).toBe('Alpha');
    source(ID_A, 'Alpha');

    type(ID_A, 'Alpha 1');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS / 2);
    type(ID_A, 'Alpha 12');
    expect(useWorkspace.getState().open[ID_A]?.state).toBe('pending');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS - 1);
    expect(backend.saves).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    await vi.waitFor(() => {
      expect(useWorkspace.getState().open[ID_A]?.state).toBe('saved');
    });
    expect(backend.saves).toEqual(['Alpha 12']);
  });

  it('saves the previous tab when switching tabs', async () => {
    const workspace = useWorkspace.getState();
    await workspace.openDocument(ID_A);
    type(ID_A, 'geändert');
    await workspace.openDocument(ID_B);
    await vi.waitFor(() => {
      expect(backend.saves).toEqual(['geändert']);
    });
    expect(useWorkspace.getState().active).toBe(ID_B);
  });

  it('keeps the change and the tab when saving fails', async () => {
    const workspace = useWorkspace.getState();
    await workspace.openDocument(ID_A);
    backend.failSave = true;
    type(ID_A, 'nicht verlieren');
    expect(await workspace.closeTab(ID_A)).toBe(false);
    const document = useWorkspace.getState().open[ID_A];
    expect(document?.state).toBe('error');
    expect(document?.savedText).toBe('Alpha');
    expect(useWorkspace.getState().tabs).toEqual([ID_A]);
    expect(useWorkspace.getState().lastError?.code).toBe('FILE_SYSTEM');

    backend.failSave = false;
    expect(await workspace.closeTab(ID_A)).toBe(true);
    expect(backend.texts.get(ID_A)).toBe('nicht verlieren');
    expect(useWorkspace.getState().tabs).toEqual([]);
  });

  it('writes nothing for unchanged text', async () => {
    const workspace = useWorkspace.getState();
    await workspace.openDocument(ID_A);
    type(ID_A, 'Alpha!');
    type(ID_A, 'Alpha');
    expect(await workspace.flushAll()).toBe(true);
    expect(useWorkspace.getState().open[ID_A]?.state).toBe('saved');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS * 2);
    expect(backend.saves).toEqual([]);
  });

  it('does not save what Markdown would lose until the user saves anyway', async () => {
    const workspace = useWorkspace.getState();
    await workspace.openDocument(ID_A);
    const fake = source(ID_A, 'Alpha');
    fake.losses = [{ kind: 'tableMergedCells', block: 2, excerpt: 'A' }];
    type(ID_A, 'Alpha | B');
    expect(await workspace.flush(ID_A)).toBe(false);
    expect(useWorkspace.getState().open[ID_A]?.state).toBe('blocked');
    expect(useWorkspace.getState().open[ID_A]?.losses).toEqual(fake.losses);
    expect(backend.saves).toEqual([]);

    expect(await workspace.saveAnyway(ID_A)).toBe(true);
    expect(fake.accepted).toEqual(fake.losses);
    expect(backend.saves).toEqual(['Alpha | B']);
    expect(useWorkspace.getState().open[ID_A]?.state).toBe('saved');
  });

  it('moves and closes tabs, activating a neighbour', async () => {
    const workspace = useWorkspace.getState();
    await workspace.openDocument(ID_A);
    await workspace.openDocument(ID_B);
    workspace.moveTab(ID_B, 0);
    expect(useWorkspace.getState().tabs).toEqual([ID_B, ID_A]);
    await workspace.closeTab(ID_B);
    expect(useWorkspace.getState().active).toBe(ID_A);
  });

  it('creates and renames documents', async () => {
    const workspace = useWorkspace.getState();
    const id = await workspace.createDocument('Unbenannt');
    expect(id).toBe(ID_B);
    expect(useWorkspace.getState().active).toBe(ID_B);
    expect(await workspace.rename(ID_B, 'Kapitel')).toBe(true);
    expect(useWorkspace.getState().open[ID_B]?.title).toBe('Kapitel');
    expect(useWorkspace.getState().documents.find((d) => d.id === ID_B)?.title).toBe('Kapitel');
  });
});

describe('freeTitle', () => {
  it('numbers titles that exist already', () => {
    const list = [summary(ID_A, 'Unbenannt'), summary(ID_B, 'Unbenannt 2')] as never[];
    expect(freeTitle('Unbenannt', list)).toBe('Unbenannt 3');
    expect(freeTitle('Notiz', list)).toBe('Notiz');
  });
});
