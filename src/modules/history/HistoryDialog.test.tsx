import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { comparisonText, HistoryDialog, useHistory } from './HistoryDialog';

const ID = '1f8a9a4e-6c39-4d6b-9b7e-6c0f5d2a1b3c';
const VERSION = '9a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d';

const initial = useWorkspace.getState();

describe('HistoryDialog', () => {
  beforeEach(() => {
    useLocale.setState({ locale: 'de' });
    mockIPC((command) => {
      switch (command) {
        case 'document_versions':
          return [
            {
              id: VERSION,
              documentId: ID,
              kind: 'named',
              name: 'Erste Fassung',
              format: 'md',
              sizeBytes: 20,
              createdAt: '2026-11-05T09:00:00.000Z',
            },
          ];
        case 'document_version_text':
          return {
            version: {
              id: VERSION,
              documentId: ID,
              kind: 'named',
              name: 'Erste Fassung',
              format: 'md',
              sizeBytes: 20,
              createdAt: '2026-11-05T09:00:00.000Z',
            },
            text: 'Kapitel\nAlter Satz.\nEnde\n',
          };
        case 'document_read':
          return {
            document: { id: ID, title: 'Roman', format: 'md' },
            text: 'Kapitel\nNeuer Satz.\nEnde\n',
            readOnly: false,
          };
        default:
          throw new Error(`unexpected ${command}`);
      }
    });
  });

  afterEach(() => {
    clearMocks();
    useHistory.setState({ documentId: null, naming: null });
    useWorkspace.setState(initial, true);
  });

  it('compares a version with the current state side by side and restores it', async () => {
    const restoreVersion = vi.fn(() => Promise.resolve(true));
    useWorkspace.setState({ restoreVersion });
    useHistory.getState().open(ID);
    render(<HistoryDialog />);
    await waitFor(() => {
      expect(screen.getByTestId('history-table')).toBeTruthy();
    });
    expect(screen.getByTestId('history-counts').textContent).toBe('1 Zeilen entfernt, 1 hinzugefügt');
    const removed = document.querySelector('[data-line="removed"]');
    const added = document.querySelector('[data-line="added"]');
    expect(removed?.textContent).toContain('Alter Satz.');
    expect(added?.textContent).toContain('Neuer Satz.');
    const left = screen.getByTestId<HTMLSelectElement>('history-left');
    expect(left.selectedOptions[0]?.textContent).toContain('Erste Fassung');
    fireEvent.click(screen.getByTestId('history-restore'));
    await waitFor(() => {
      expect(restoreVersion).toHaveBeenCalledWith(ID, VERSION);
    });
    await waitFor(() => {
      expect(useHistory.getState().documentId).toBeNull();
    });
  });

  it('compares rich text as Markdown', () => {
    const content = JSON.stringify({
      type: 'doc',
      content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Fett', marks: [{ type: 'bold' }] }] }],
    });
    expect(comparisonText('pwrite', content)).toBe('**Fett**\n');
    expect(comparisonText('txt', 'roh')).toBe('roh');
  });
});
