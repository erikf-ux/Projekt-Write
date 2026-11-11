import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SearchHit } from '@/types/generated/SearchHit';
import type { SearchRequest } from '@/types/generated/SearchRequest';
import { firstHitText } from './openHit';
import { SearchPanel } from './SearchPanel';
import { localMidnight, NO_FILTERS, SEARCH_DELAY_MS, searchRequest, useSearch } from './searchStore';

const openHit = vi.hoisted(() => vi.fn());
vi.mock('./openHit', async (original) => ({
  ...(await original<typeof import('./openHit')>()),
  openHit,
}));

function hit(id: string, title: string, extra: Partial<SearchHit> = {}): SearchHit {
  return {
    kind: 'document',
    id,
    title,
    format: 'md',
    updatedAt: '2026-11-11T10:00:00.000Z',
    archived: false,
    deleted: false,
    favorite: false,
    snippet: [
      { text: 'Sehr geehrter Herr ', hit: false },
      { text: 'Müller', hit: true },
    ],
    match: 'words',
    ...extra,
  };
}

describe('search requests', () => {
  it('sends only the filters that are set', () => {
    expect(searchRequest('Müller', NO_FILTERS)).toEqual({ query: 'Müller', limit: 50 });
    const request = searchRequest('haus', {
      kinds: ['document', 'pdf'],
      tagIds: ['t1'],
      subnetId: 's1',
      from: '2026-11-01',
      to: '2026-11-10',
      favoritesOnly: true,
      includeArchived: true,
      includeTrash: true,
    });
    expect(request).toEqual({
      query: 'haus',
      limit: 50,
      kinds: ['document', 'pdf'],
      tagIds: ['t1'],
      subnetId: 's1',
      updatedFrom: new Date(2026, 10, 1).toISOString(),
      // "bis" includes the whole day.
      updatedBefore: new Date(2026, 10, 11).toISOString(),
      favoritesOnly: true,
      includeArchived: true,
      includeTrash: true,
    });
  });

  it('reads local dates and refuses anything else', () => {
    expect(localMidnight('2026-02-28', 1)).toBe(new Date(2026, 2, 1).toISOString());
    expect(localMidnight('')).toBeNull();
    expect(localMidnight('gestern')).toBeNull();
  });

  it('takes the first marked word of a snippet for the find bar', () => {
    expect(firstHitText(hit('a', 'Brief'))).toBe('Müller');
    expect(firstHitText(hit('a', 'Brief', { snippet: [{ text: 'nur Text', hit: false }] }))).toBeNull();
  });
});

describe('search store and panel', () => {
  let requests: SearchRequest[];
  let answers: Record<string, SearchHit[]>;

  beforeEach(() => {
    vi.useFakeTimers();
    requests = [];
    answers = {};
    useSearch.getState().reset();
    openHit.mockReset();
    mockIPC((command, args) => {
      if (command === 'search_query') {
        const request = (args as { request: SearchRequest }).request;
        requests.push(request);
        return answers[request.query] ?? [];
      }
      return null;
    });
  });

  afterEach(() => {
    clearMocks();
    vi.useRealTimers();
  });

  async function settle() {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(SEARCH_DELAY_MS + 10);
    });
  }

  it('searches a moment after the last keystroke', async () => {
    answers['Mül'] = [hit('a', 'Brief')];
    const store = useSearch.getState();
    store.setQuery('M');
    store.setQuery('Mü');
    store.setQuery('Mül');
    await settle();
    expect(requests.map((request) => request.query)).toEqual(['Mül']);
    expect(useSearch.getState().status).toBe('done');
    expect(useSearch.getState().hits).toHaveLength(1);
    expect(useSearch.getState().selected).toBe(0);
  });

  it('drops an answer that a newer search overtook', async () => {
    let release: (() => void) | null = null;
    mockIPC(async (command, args) => {
      if (command !== 'search_query') {
        return null;
      }
      const request = (args as { request: SearchRequest }).request;
      if (request.query === 'alt') {
        await new Promise<void>((resolve) => {
          release = resolve;
        });
        return [hit('old', 'Alt')];
      }
      return [hit('new', 'Neu')];
    });
    useSearch.getState().setQuery('alt');
    await settle();
    useSearch.getState().setQuery('neu');
    await settle();
    await act(async () => {
      release?.();
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(useSearch.getState().hits.map((entry) => entry.id)).toEqual(['new']);
  });

  it('shows nothing and asks nothing for an empty field', async () => {
    useSearch.getState().setQuery('   ');
    await settle();
    expect(requests).toEqual([]);
    expect(useSearch.getState().status).toBe('idle');
  });

  it('shows hits with the marked word and opens them by keyboard and click', async () => {
    answers['Müller'] = [
      hit('a', 'Brief'),
      hit('b', 'Altes Protokoll', { archived: true, match: 'parts' }),
      hit('c', 'Reise', { kind: 'subnet', format: null, snippet: [] }),
    ];
    render(<SearchPanel />);
    const field = screen.getByTestId('search-field');
    fireEvent.change(field, { target: { value: 'Müller' } });
    await settle();

    expect(screen.getByTestId('search-status').textContent).toContain('3');
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(3);
    expect(options[0]?.querySelector('mark')?.textContent).toBe('Müller');
    expect(options[0]?.getAttribute('aria-selected')).toBe('true');
    expect(options[1]?.getAttribute('data-match')).toBe('parts');
    expect(screen.getByLabelText('Archived')).toBeTruthy();

    fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(useSearch.getState().selected).toBe(1);
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    // The end of the list stops the selection.
    expect(useSearch.getState().selected).toBe(2);
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(openHit).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'c' }));

    fireEvent.click(options[0] as HTMLElement);
    expect(openHit).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'a' }));
    expect(useSearch.getState().selected).toBe(0);

    // Escape empties the field.
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(useSearch.getState().query).toBe('');
  });

  it('runs again at once when a filter changes, and says when nothing matches', async () => {
    render(<SearchPanel />);
    fireEvent.change(screen.getByTestId('search-field'), { target: { value: 'Garten' } });
    await settle();
    expect(screen.getByTestId('search-status').textContent).toBe('No results');

    fireEvent.click(screen.getByTestId('search-filter-toggle'));
    fireEvent.click(screen.getByTestId('search-filter-archive'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(requests.at(-1)).toEqual({ query: 'Garten', limit: 50, includeArchived: true });
    fireEvent.click(screen.getByTestId('search-filter-clear'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(requests.at(-1)).toEqual({ query: 'Garten', limit: 50 });
  });

  it('shows the error of a failed search', async () => {
    mockIPC((command) => {
      if (command === 'search_query') {
        throw { code: 'LIBRARY_NOT_OPEN', messageKey: 'error.libraryNotOpen' };
      }
      return null;
    });
    render(<SearchPanel />);
    fireEvent.change(screen.getByTestId('search-field'), { target: { value: 'x' } });
    await settle();
    expect(screen.getByTestId('search-status').getAttribute('data-status')).toBe('error');
  });
});
