import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useBackup } from '@/modules/backup/backupStore';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { panelElementId } from '@/modules/editor/DocumentEditor';
import { useLibrary } from '@/modules/library/libraryStore';
import { useUi } from '@/modules/settings/uiStore';
import { exportLibrary, PRINT_ROOT_ID, printActive } from './exportActions';

const ID = '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11';

function activeDocumentInWorkspace() {
  useWorkspace.setState({
    tabs: [ID],
    active: ID,
    open: { [ID]: { id: ID, title: 'Brief an Herrn Müller', loaded: true, format: 'md' } as never },
    flush: vi.fn(async () => Promise.resolve(true)),
    flushAll: vi.fn(async () => Promise.resolve(true)),
  });
}

describe('printing', () => {
  beforeEach(() => {
    activeDocumentInWorkspace();
    const panel = document.createElement('div');
    panel.id = panelElementId(ID);
    const editor = document.createElement('div');
    editor.setAttribute('data-testid', 'editor');
    editor.setAttribute('contenteditable', 'true');
    editor.innerHTML =
      '<h1>Hafen</h1><p contenteditable="false">Text mit <a href="https://example.org">Link</a></p>';
    panel.append(editor);
    document.body.append(panel);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('prints a copy of the writing surface with its title and cleans up afterwards', async () => {
    let seen: { title: string; printing: boolean; text: string; editable: number } | null = null;
    vi.spyOn(window, 'print').mockImplementation(() => {
      const root = document.getElementById(PRINT_ROOT_ID);
      seen = {
        title: document.title,
        printing: document.documentElement.hasAttribute('data-printing'),
        text: root?.textContent ?? '',
        editable: root?.querySelectorAll('[contenteditable]').length ?? -1,
      };
    });
    document.title = 'Project Write';
    expect(await printActive(false)).toBe(true);
    expect(seen).toEqual({
      title: 'Brief an Herrn Müller',
      printing: true,
      text: 'Brief an Herrn MüllerHafenText mit Link',
      editable: 0,
    });
    // Until the print dialog is closed the copy stays.
    window.dispatchEvent(new Event('afterprint'));
    expect(document.getElementById(PRINT_ROOT_ID)).toBeNull();
    expect(document.documentElement.hasAttribute('data-printing')).toBe(false);
    expect(document.title).toBe('Project Write');
  });

  it('names the PDF printer for "Als PDF exportieren"', async () => {
    vi.spyOn(window, 'print').mockImplementation(() => undefined);
    await printActive(true);
    expect(useUi.getState().notice?.key).toBe('export.pdfHint');
    window.dispatchEvent(new Event('afterprint'));
  });

  it('prints nothing without an open document', async () => {
    useWorkspace.setState({ active: null });
    const print = vi.spyOn(window, 'print').mockImplementation(() => undefined);
    expect(await printActive(false)).toBe(false);
    expect(print).not.toHaveBeenCalled();
  });
});

describe('library export', () => {
  let calls: { command: string; args: unknown }[];

  beforeEach(() => {
    activeDocumentInWorkspace();
    calls = [];
    mockIPC((command, args) => {
      calls.push({ command, args });
      switch (command) {
        case 'export_library_begin':
          return { exportId: 'e1', toConvert: [{ id: ID, title: 'Rich' }], written: 3 };
        case 'document_read':
          return {
            document: { id: ID },
            text: JSON.stringify({
              type: 'doc',
              content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Hallo **Welt**' }] }],
            }),
            readOnly: false,
          };
        case 'export_library_add':
          return null;
        case 'export_library_finish':
          return { location: '/x/Project Write Export', documents: 4, media: 0 };
        case 'export_library_cancel':
          return true;
        default:
          return null;
      }
    });
  });

  afterEach(() => {
    clearMocks();
  });

  it('converts each .pwrite document to Markdown and finishes', async () => {
    const progress: [number, number][] = [];
    expect(
      await exportLibrary(({ done, total }) => {
        progress.push([done, total]);
      }),
    ).toBe(true);
    const add = calls.find((entry) => entry.command === 'export_library_add');
    const request = (add?.args as { request: { exportId: string; id: string; markdown: string } }).request;
    expect(request.exportId).toBe('e1');
    expect(request.id).toBe(ID);
    expect(request.markdown).toContain('Hallo');
    expect(progress).toEqual([
      [3, 4],
      [4, 4],
    ]);
    expect(calls.at(-1)?.command).toBe('export_library_finish');
    expect(useUi.getState().notice?.key).toBe('export.libraryDone');
  });

  it('cancels when a document cannot be read', async () => {
    mockIPC((command) => {
      calls.push({ command, args: null });
      if (command === 'export_library_begin') {
        return { exportId: 'e2', toConvert: [{ id: ID, title: 'Rich' }], written: 0 };
      }
      if (command === 'document_read') {
        throw { code: 'DOCUMENT_NOT_FOUND', messageKey: 'error.documentNotFound' };
      }
      return true;
    });
    expect(await exportLibrary()).toBe(false);
    expect(calls.map((entry) => entry.command)).toContain('export_library_cancel');
    expect(calls.map((entry) => entry.command)).not.toContain('export_library_finish');
  });
});

describe('backups', () => {
  afterEach(() => {
    clearMocks();
  });

  it('reports a backup, a cancelled one and a restore of stage 5', async () => {
    const overview = {
      target: '/lib/backups',
      defaultTarget: true,
      sameDrive: false,
      automatic: true,
      keep: 7,
      running: false,
      lastBackupAt: null,
    };
    let answer: 'ok' | 'cancelled' = 'ok';
    mockIPC((command) => {
      switch (command) {
        case 'backup_overview':
          return overview;
        case 'backup_create':
          if (answer === 'cancelled') {
            throw { code: 'BACKUP_CANCELLED', messageKey: 'error.backupCancelled' };
          }
          return {
            createdAt: '2026-11-12T10:00:00.000Z',
            appVersion: '0.1.0',
            schemaVersion: 9,
            libraryId: ID,
            documents: 3,
            media: 2,
            bytes: 100,
          };
        case 'backup_restore':
          return { state: 'open', library: { displayPath: '/neu' } };
        default:
          return null;
      }
    });
    expect(await useBackup.getState().create()).toBe(true);
    expect(useUi.getState().notice).toMatchObject({ key: 'backup.done', params: { documents: 3, media: 2 } });
    expect(useBackup.getState().overview).toEqual(overview);

    answer = 'cancelled';
    expect(await useBackup.getState().create()).toBe(false);
    expect(useUi.getState().notice).toMatchObject({ tone: 'info', key: 'backup.cancelled' });
    expect(useBackup.getState().busy).toBe(false);

    expect(await useBackup.getState().restore()).toBe(true);
    expect(useLibrary.getState().status).toMatchObject({ state: 'open' });
    expect(useUi.getState().notice?.key).toBe('backup.restored');
  });
});
