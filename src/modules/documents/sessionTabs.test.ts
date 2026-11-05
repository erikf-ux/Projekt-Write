import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  restoreSession,
  saveSessionNow,
  SESSION_SAVE_DELAY_MS,
  startSessionTracking,
  stopSessionTracking,
  takePendingSelection,
} from './sessionTabs';
import { useWorkspace } from './workspaceStore';

const A = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';
const B = '2f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';

function summary(id: string) {
  return {
    id,
    title: id === A ? 'Eins' : 'Zwei',
    format: 'md',
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

describe('session (stage 1)', () => {
  const saved: unknown[] = [];

  beforeEach(async () => {
    vi.useFakeTimers();
    saved.length = 0;
    mockIPC((command, args) => {
      const request = (args as { request: Record<string, unknown> }).request;
      switch (command) {
        case 'documents_list':
          return [summary(A), summary(B)];
        case 'document_read':
          return { document: summary(String(request['id'])), text: 'Text\n', readOnly: false };
        case 'session_restore':
          return {
            tabs: [
              { target: { kind: 'library', id: A }, anchor: 3, head: 4, view: 'source' },
              { target: { kind: 'library', id: B }, anchor: 2, head: 2, view: 'rich' },
            ],
            active: 0,
          };
        case 'session_save':
          saved.push(request);
          return null;
        default:
          return null;
      }
    });
    await useWorkspace.getState().reset();
    await useWorkspace.getState().refresh();
  });

  afterEach(() => {
    stopSessionTracking();
    vi.useRealTimers();
    clearMocks();
  });

  it('opens the tabs of the last session with their selection and the active tab', async () => {
    await restoreSession();
    const state = useWorkspace.getState();
    expect(state.tabs).toEqual([A, B]);
    expect(state.active).toBe(A);
    // Saving before the editors exist keeps what was restored.
    startSessionTracking();
    await saveSessionNow();
    expect(saved).toEqual([
      {
        tabs: [
          { target: { kind: 'library', id: A }, anchor: 3, head: 4, view: 'source' },
          { target: { kind: 'library', id: B }, anchor: 2, head: 2, view: 'rich' },
        ],
        active: 0,
      },
    ]);
    expect(takePendingSelection(A)).toEqual({ anchor: 3, head: 4, view: 'source' });
    expect(takePendingSelection(A)).toBeUndefined();
  });

  it('saves 2 s after the tabs change, and not before it was restored', async () => {
    await useWorkspace.getState().openDocument(A);
    await vi.advanceTimersByTimeAsync(SESSION_SAVE_DELAY_MS * 2);
    expect(saved).toEqual([]);
    startSessionTracking();
    await useWorkspace.getState().openDocument(B);
    await vi.advanceTimersByTimeAsync(SESSION_SAVE_DELAY_MS - 1);
    expect(saved).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(saved).toHaveLength(1);
    expect((saved[0] as { active: number }).active).toBe(1);
  });
});
