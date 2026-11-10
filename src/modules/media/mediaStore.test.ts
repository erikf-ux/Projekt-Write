import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { isDocumentFile } from '@/modules/documents/importFiles';
import type { ImportPreview } from '@/types/generated/ImportPreview';
import { useMedia } from './mediaStore';

function preview(batchId: string, name: string, duplicate = false): ImportPreview {
  return {
    batchId,
    items: [
      {
        index: 0,
        name,
        folder: '',
        kind: 'image',
        sizeBytes: 4,
        duplicate: duplicate ? { kind: 'existing', id: 'old', title: 'Alt' } : null,
      },
    ],
    skipped: [],
    folderName: null,
  };
}

describe('dropped media', () => {
  let commits: string[];
  let cancels: string[];

  beforeEach(() => {
    commits = [];
    cancels = [];
    useMedia.getState().reset();
    const previews: Record<number, ImportPreview> = {
      0: preview('a', 'A.png'),
      1: preview('b', 'B.png', true),
      2: preview('c', 'C.png'),
    };
    let prepared = 0;
    mockIPC((command, args) => {
      switch (command) {
        case 'media_import_bytes':
          prepared += 1;
          return previews[prepared - 1];
        case 'media_import_commit': {
          const batchId = (args as { request: { batchId: string } }).request.batchId;
          commits.push(batchId);
          return { items: [{ index: 0, id: `m-${batchId}`, created: true }], subnetId: null };
        }
        case 'media_import_cancel':
          cancels.push((args as { request: { batchId: string } }).request.batchId);
          return null;
        case 'media_list':
        case 'media_trash_list':
          return [];
        case 'structure_state':
          return { subnets: [], tags: [], documents: [], media: [], history: { undo: null, redo: null } };
        default:
          throw new Error(command);
      }
    });
  });

  afterEach(() => {
    clearMocks();
  });

  it('imports at once, asks about a duplicate and goes on with the rest after the dialog', async () => {
    const files = ['A.png', 'B.png', 'C.png'].map((name) => new File(['data'], name));
    const outcome = await useMedia.getState().importFiles(files);
    expect(outcome?.items.map((item) => item.id)).toEqual(['m-a']);
    expect(commits).toEqual(['a']);
    const state = useMedia.getState().importing;
    expect(state?.phase).toBe('review');
    expect(state?.phase === 'review' ? state.preview.batchId : null).toBe('b');

    // Leaving out the duplicate: C follows without another drop.
    await useMedia.getState().cancelImport();
    expect(cancels).toEqual(['b']);
    await vi.waitFor(() => {
      expect(commits).toEqual(['a', 'c']);
    });
    expect(useMedia.getState().importing).toBeNull();
  });

  it('tells text documents from media by their ending', () => {
    expect(isDocumentFile('Kapitel.md')).toBe(true);
    expect(isDocumentFile('Notiz.TXT')).toBe(true);
    expect(isDocumentFile('Roman.pwrite')).toBe(true);
    expect(isDocumentFile('Bild.png')).toBe(false);
    expect(isDocumentFile('.md')).toBe(false);
    expect(isDocumentFile('Brief.docx')).toBe(false);
  });
});
