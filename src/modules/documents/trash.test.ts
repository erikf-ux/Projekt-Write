import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePrompt } from '@/components/prompt/promptStore';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { setDocumentSource, useWorkspace } from './workspaceStore';

const ID = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

function summary(extra: Partial<DocumentSummary> = {}): DocumentSummary {
  return {
    id: ID,
    title: 'Kapitel',
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-05T09:00:00.000Z',
    updatedAt: '2026-11-05T09:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
    ...extra,
  };
}

async function answer(choice: string): Promise<void> {
  await vi.waitFor(() => {
    expect(usePrompt.getState().queue.length).toBeGreaterThan(0);
  });
  usePrompt.getState().answer(choice);
}

describe('trash and archive', () => {
  const calls: string[] = [];
  let text = 'Text\n';

  beforeEach(async () => {
    calls.length = 0;
    text = 'Text\n';
    mockIPC((command) => {
      calls.push(command);
      switch (command) {
        case 'documents_list':
          return [summary()];
        case 'documents_trash':
          return [];
        case 'document_read':
          return { document: summary(), text, readOnly: false };
        case 'document_save':
          return summary();
        case 'document_delete':
          return summary({ deletedAt: '2026-11-05T10:00:00.000Z' });
        case 'document_restore':
          return summary();
        case 'document_archive':
          return summary({ archivedAt: '2026-11-05T10:00:00.000Z' });
        case 'document_purge':
          return null;
        case 'trash_empty':
          return 1;
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
    await useWorkspace.getState().reset();
    await useWorkspace.getState().refresh();
  });

  afterEach(() => {
    clearMocks();
    usePrompt.setState({ queue: [] });
  });

  it('saves an open document, closes its tab and moves it to the trash', async () => {
    await useWorkspace.getState().openDocument(ID);
    setDocumentSource(ID, {
      saveRequest: () => ({ kind: 'text', text: 'Neu\n' }),
      acceptLosses: () => undefined,
    });
    useWorkspace.getState().edit(ID);
    expect(await useWorkspace.getState().deleteDocument(ID)).toBe(true);
    expect(calls.indexOf('document_save')).toBeLessThan(calls.indexOf('document_delete'));
    const state = useWorkspace.getState();
    expect(state.tabs).toEqual([]);
    expect(state.documents).toEqual([]);
    expect(state.trash.map((entry) => entry.id)).toEqual([ID]);

    expect(await useWorkspace.getState().restoreDocument(ID)).toBe(true);
    expect(useWorkspace.getState().trash).toEqual([]);
    expect(useWorkspace.getState().documents.map((entry) => entry.id)).toEqual([ID]);
  });

  it('archives without touching the content', async () => {
    expect(await useWorkspace.getState().archiveDocument(ID, true)).toBe(true);
    expect(useWorkspace.getState().documents[0]?.archivedAt).toBe('2026-11-05T10:00:00.000Z');
    expect(calls).not.toContain('document_save');
  });

  it('asks before deleting for good and before emptying the trash', async () => {
    await useWorkspace.getState().deleteDocument(ID);
    const cancelled = useWorkspace.getState().purgeDocument(ID);
    await answer('cancel');
    expect(await cancelled).toBe(false);
    expect(calls).not.toContain('document_purge');

    const purged = useWorkspace.getState().purgeDocument(ID);
    await answer('purge');
    expect(await purged).toBe(true);
    expect(useWorkspace.getState().trash).toEqual([]);

    await useWorkspace.getState().deleteDocument(ID);
    const emptied = useWorkspace.getState().emptyTrash();
    await answer('empty');
    expect(await emptied).toBe(true);
    expect(calls).toContain('trash_empty');
    expect(useWorkspace.getState().trash).toEqual([]);
    // Nothing to empty: no question.
    expect(await useWorkspace.getState().emptyTrash()).toBe(false);
  });
});
