import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AUTOSAVE_DELAY_MS, freeTitle, useWorkspace } from './workspaceStore';

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
  });

  it('autosaves 1 s after the last input, once', async () => {
    const workspace = useWorkspace.getState();
    expect(await workspace.openDocument(ID_A)).toBe(true);
    expect(useWorkspace.getState().open[ID_A]?.text).toBe('Alpha');

    workspace.edit(ID_A, 'Alpha 1');
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS / 2);
    workspace.edit(ID_A, 'Alpha 12');
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
    workspace.edit(ID_A, 'geändert');
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
    workspace.edit(ID_A, 'nicht verlieren');
    expect(await workspace.closeTab(ID_A)).toBe(false);
    const document = useWorkspace.getState().open[ID_A];
    expect(document?.state).toBe('error');
    expect(document?.text).toBe('nicht verlieren');
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
    workspace.edit(ID_A, 'Alpha!');
    workspace.edit(ID_A, 'Alpha');
    expect(useWorkspace.getState().open[ID_A]?.state).toBe('saved');
    expect(await workspace.flushAll()).toBe(true);
    await vi.advanceTimersByTimeAsync(AUTOSAVE_DELAY_MS * 2);
    expect(backend.saves).toEqual([]);
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
