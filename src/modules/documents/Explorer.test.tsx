import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '@/i18n';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { useStructure } from '@/modules/structure/structureStore';
import { copyTitle, Explorer, RECENT_LIMIT } from './Explorer';
import { explorerModel, NO_FILTER } from './explorerRows';
import { useExplorer } from './explorerStore';
import { useWorkspace } from './workspaceStore';

function summary(id: string, title: string, extra: Partial<DocumentSummary> = {}): DocumentSummary {
  return {
    id,
    title,
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-04T10:00:00.000Z',
    updatedAt: '2026-11-04T10:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
    ...extra,
  };
}

const initial = useWorkspace.getState();

function tree() {
  return screen.getByRole('tree');
}

function rowsOf(group: string): string[] {
  return [...tree().querySelectorAll(`[data-group-member="${group}"]`)].map((row) => row.textContent);
}

describe('Explorer', () => {
  beforeEach(() => {
    useLocale.setState({ locale: 'de' });
    useWorkspace.setState({
      documents: [
        summary('a', 'Beta', { lastOpenedAt: '2026-11-04T10:05:00.000Z' }),
        summary('b', 'Alpha', { favoritedAt: '2026-11-04T09:00:00.000Z' }),
        summary('c', 'Gamma', { lastOpenedAt: '2026-11-04T10:06:00.000Z' }),
      ],
      listLoaded: true,
      active: null,
      tabs: [],
      open: {},
    });
  });

  afterEach(() => {
    useWorkspace.setState(initial, true);
    useStructure.getState().reset();
    useExplorer.getState().reset();
  });

  it('shows favourites, recently used and the documents in no subnet as one tree', () => {
    render(<Explorer onNewDocument={() => undefined} />);
    const groups = within(tree())
      .getAllByRole('treeitem')
      .filter((row) => row.getAttribute('aria-level') === '1');
    expect(groups.map((row) => row.textContent)).toEqual([
      'Favoriten1',
      'Zuletzt verwendet2',
      'Nicht zugeordnet3',
      'Medien0',
      'Archiv0',
      'Papierkorb0',
    ]);
    expect(rowsOf('favorites')).toEqual(['Alpha']);
    expect(rowsOf('recent')).toEqual(['Gamma', 'Beta']);
    expect(rowsOf('unassigned')).toEqual(['Alpha', 'Beta', 'Gamma']);
    const alpha = tree().querySelector<HTMLElement>('[data-group-member="unassigned"][data-document="b"]');
    expect(alpha && within(alpha).getByRole('img', { name: 'Favorit' })).toBeTruthy();
    // One element in several entries.
    expect(tree().querySelectorAll('[data-document="b"]')).toHaveLength(2);
  });

  it('opens and closes entries with the arrow keys and opens documents with Enter', () => {
    const openDocument = vi.fn(() => Promise.resolve(true));
    useWorkspace.setState({ openDocument });
    render(<Explorer onNewDocument={() => undefined} />);
    act(() => {
      tree().focus();
    });
    const key = (name: string) => {
      fireEvent.keyDown(tree(), { key: name });
    };
    // The first row is "Favoriten".
    key('ArrowLeft');
    expect(rowsOf('favorites')).toEqual([]);
    expect(tree().querySelector('[data-group="favorites"]')?.getAttribute('aria-expanded')).toBe('false');
    key('ArrowRight');
    expect(rowsOf('favorites')).toEqual(['Alpha']);
    key('ArrowRight');
    key('Enter');
    expect(openDocument).toHaveBeenCalledWith('b');
    // Left from a document goes to its entry.
    key('ArrowLeft');
    expect(tree().getAttribute('aria-activedescendant')).toBe('pw-explorer-favorites');
  });

  it('makes copies and favourites from the context menu', () => {
    const copyDocument = vi.fn(() => Promise.resolve('d'));
    const setFavorite = vi.fn(() => Promise.resolve(true));
    useWorkspace.setState({ copyDocument, setFavorite });
    render(<Explorer onNewDocument={() => undefined} />);
    const beta = tree().querySelector('[data-group-member="unassigned"][data-document="a"]');
    if (!beta) {
      throw new Error('no row');
    }
    fireEvent.contextMenu(beta);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Kopie anlegen' }));
    expect(copyDocument).toHaveBeenCalledWith('a', 'Beta (Kopie)');
    fireEvent.contextMenu(beta);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Zu den Favoriten' }));
    expect(setFavorite).toHaveBeenCalledWith('a', true);
  });

  it('shows archived documents only under "Archiv" and deleted ones in the trash', () => {
    const archiveDocument = vi.fn(() => Promise.resolve(true));
    const deleteDocument = vi.fn(() => Promise.resolve(true));
    const restoreDocument = vi.fn(() => Promise.resolve(true));
    const openDocument = vi.fn(() => Promise.resolve(true));
    useWorkspace.setState({
      documents: [
        summary('a', 'Aktiv', { lastOpenedAt: '2026-11-04T10:05:00.000Z' }),
        summary('b', 'Alt', {
          archivedAt: '2026-11-04T09:00:00.000Z',
          favoritedAt: '2026-11-04T09:00:00.000Z',
          lastOpenedAt: '2026-11-04T10:06:00.000Z',
        }),
      ],
      trash: [summary('t', 'Weg', { deletedAt: '2026-11-04T11:00:00.000Z' })],
      archiveDocument,
      deleteDocument,
      restoreDocument,
      openDocument,
    });
    render(<Explorer onNewDocument={() => undefined} />);
    expect(rowsOf('favorites')).toEqual([]);
    expect(rowsOf('recent')).toEqual(['Aktiv']);
    expect(rowsOf('unassigned')).toEqual(['Aktiv']);
    // Archive and trash start closed.
    expect(rowsOf('archive')).toEqual([]);
    const list = tree();
    list.focus();
    fireEvent.click(within(list).getByText('Archiv'));
    fireEvent.click(within(list).getByText('Papierkorb'));
    expect(rowsOf('archive')).toEqual(['Alt']);
    expect(rowsOf('trash')).toEqual(['Weg']);

    // A document in the trash does not open; its menu restores it.
    const trashed = list.querySelector<HTMLElement>('[data-group-member="trash"][data-document="t"]');
    expect(trashed?.getAttribute('title')).toContain('Im Papierkorb seit');
    fireEvent.click(trashed as HTMLElement);
    expect(openDocument).not.toHaveBeenCalled();
    fireEvent.contextMenu(trashed as HTMLElement);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Wiederherstellen (Stufe 3)' }));
    expect(restoreDocument).toHaveBeenCalledWith('t');

    // Del moves the selected document to the trash; the menu archives.
    const active = list.querySelector<HTMLElement>('[data-group-member="unassigned"][data-document="a"]');
    fireEvent.contextMenu(active as HTMLElement);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Archivieren' }));
    expect(archiveDocument).toHaveBeenCalledWith('a', true);
    fireEvent.keyDown(list, { key: 'Delete' });
    expect(deleteDocument).toHaveBeenCalledWith('a');
  });

  it('lists the most recently opened documents only', () => {
    const many = Array.from({ length: RECENT_LIMIT + 3 }, (_, index) =>
      summary(`d${String(index)}`, `D${String(index)}`, {
        lastOpenedAt: `2026-11-04T10:${String(index).padStart(2, '0')}:00.000Z`,
      }),
    );
    const { recent } = explorerModel({
      documents: many,
      trash: [],
      recentFiles: [],
      subnets: [],
      placements: {},
      filter: NO_FILTER,
      locale: 'de',
    });
    expect(recent).toHaveLength(RECENT_LIMIT);
    expect(recent[0]?.kind === 'document' && recent[0].document.id).toBe(`d${String(RECENT_LIMIT + 2)}`);
    expect(copyTitle('x'.repeat(300), ' (Kopie)')).toHaveLength(300);
  });
});
