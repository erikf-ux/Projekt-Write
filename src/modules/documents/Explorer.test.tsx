import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '@/i18n';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { copyTitle, Explorer, explorerGroups, RECENT_LIMIT } from './Explorer';
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
  });

  it('shows favourites, recently used and all documents as one tree', () => {
    render(<Explorer onNewDocument={() => undefined} />);
    const groups = within(tree())
      .getAllByRole('treeitem')
      .filter((row) => row.getAttribute('aria-level') === '1');
    expect(groups.map((row) => row.textContent)).toEqual([
      'Favoriten1',
      'Zuletzt verwendet2',
      'Alle Dokumente3',
    ]);
    expect(rowsOf('favorites')).toEqual(['Alpha']);
    expect(rowsOf('recent')).toEqual(['Gamma', 'Beta']);
    expect(rowsOf('all')).toEqual(['Alpha', 'Beta', 'Gamma']);
    const alpha = tree().querySelector<HTMLElement>('[data-group-member="all"][data-document="b"]');
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
    const beta = tree().querySelector('[data-group-member="all"][data-document="a"]');
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

  it('lists the most recently opened documents only', () => {
    const many = Array.from({ length: RECENT_LIMIT + 3 }, (_, index) =>
      summary(`d${String(index)}`, `D${String(index)}`, {
        lastOpenedAt: `2026-11-04T10:${String(index).padStart(2, '0')}:00.000Z`,
      }),
    );
    const groups = explorerGroups(many, 'de');
    expect(groups.recent).toHaveLength(RECENT_LIMIT);
    expect(groups.recent[0]?.id).toBe(`d${String(RECENT_LIMIT + 2)}`);
    expect(copyTitle('x'.repeat(300), ' (Kopie)')).toHaveLength(300);
  });
});
