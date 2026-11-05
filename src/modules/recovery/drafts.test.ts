import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DRAFT_DELAY_MS, setDocumentSource, useWorkspace } from '@/modules/documents/workspaceStore';
import { closeAllSessions, openSession } from '@/modules/editor/session';

const ID = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

function summary() {
  return {
    id: ID,
    title: 'Roman',
    format: 'txt',
    sizeBytes: 0,
    createdAt: '2026-11-05T09:00:00.000Z',
    updatedAt: '2026-11-05T09:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
  };
}

describe('recovery drafts (stage 2)', () => {
  const drafts: { at: number; text: string }[] = [];
  const discarded: string[] = [];
  let failSave = false;

  beforeEach(async () => {
    vi.useFakeTimers();
    drafts.length = 0;
    discarded.length = 0;
    failSave = false;
    mockIPC((command, args) => {
      const request = (args as { request: Record<string, unknown> }).request;
      switch (command) {
        case 'documents_list':
          return [summary()];
        case 'documents_trash':
          return [];
        case 'document_read':
          return { document: summary(), text: 'Anfang', readOnly: false };
        case 'document_save':
          if (failSave) {
            throw { code: 'FILE_SYSTEM', messageKey: 'error.fileSystem' };
          }
          return summary();
        case 'recovery_write':
          drafts.push({ at: Date.now(), text: String(request['text']) });
          return null;
        case 'recovery_discard':
          discarded.push(JSON.stringify(request['target']));
          return null;
        default:
          return null;
      }
    });
    await useWorkspace.getState().reset();
    await useWorkspace.getState().refresh();
    await useWorkspace.getState().openDocument(ID);
    // The editor session of the tab, as DocumentEditor would create it.
    const session = openSession({
      id: ID,
      format: 'txt',
      text: 'Anfang',
      label: 'Roman',
      editable: true,
      onChange: () => {
        useWorkspace.getState().edit(ID);
      },
      onBlur: () => undefined,
    });
    setDocumentSource(ID, session);
    // What the previous test left (saved by the reset) does not count.
    drafts.length = 0;
    discarded.length = 0;
  });

  afterEach(() => {
    vi.useRealTimers();
    clearMocks();
    closeAllSessions();
  });

  it('writes a draft at least every second while typing without a pause', async () => {
    failSave = true;
    const session = (await import('@/modules/editor/session')).getSession(ID);
    // 3.3 s of typing, a key every 150 ms: autosave (1 s after the last key) never runs.
    const keys: { at: number; typed: number }[] = [];
    for (let key = 1; key <= 22; key += 1) {
      session?.editor.commands.insertContentAt(1, 'x');
      keys.push({ at: Date.now(), typed: key });
      await vi.advanceTimersByTimeAsync(150);
    }
    await vi.advanceTimersByTimeAsync(DRAFT_DELAY_MS);
    expect(drafts.length).toBeGreaterThanOrEqual(3);
    // Every key is in a draft at most a second after it was typed.
    const typedIn = (text: string) => /^x*/.exec(text)?.[0].length ?? 0;
    for (const key of keys) {
      // Drafts are in order: the first one that holds this key.
      const draft = drafts.find((entry) => typedIn(entry.text) >= key.typed);
      expect(draft, `key ${String(key.typed)}`).toBeDefined();
      expect((draft?.at ?? Infinity) - key.at).toBeLessThanOrEqual(DRAFT_DELAY_MS);
    }
  });

  it('drops the draft once the tab is saved', async () => {
    const session = (await import('@/modules/editor/session')).getSession(ID);
    session?.editor.commands.insertContentAt(1, 'Neu ');
    await vi.advanceTimersByTimeAsync(DRAFT_DELAY_MS);
    expect(drafts).toHaveLength(1);
    await useWorkspace.getState().save(ID);
    expect(discarded).toEqual([JSON.stringify({ kind: 'library', id: ID })]);
  });

  it('writes nothing for a tab without unsaved changes', async () => {
    useWorkspace.getState().edit(ID);
    await vi.advanceTimersByTimeAsync(DRAFT_DELAY_MS * 2);
    expect(drafts).toEqual([]);
  });
});
