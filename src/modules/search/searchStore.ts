/**
 * The global search in the interface (Phase 9, section 19, ADR 0020).
 *
 * Rust builds and runs every query; this store holds what the user typed,
 * the filters, the hits and the selected hit. A search starts a moment
 * after the last keystroke; an answer that arrives after a newer request
 * was sent is dropped.
 */
import { create } from 'zustand';
import { call } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { SearchHit } from '@/types/generated/SearchHit';
import type { SearchKind } from '@/types/generated/SearchKind';
import type { SearchRequest } from '@/types/generated/SearchRequest';

/** Pause after the last keystroke before searching. */
export const SEARCH_DELAY_MS = 60;
/** Hits asked for. */
export const SEARCH_LIMIT = 50;
/** Set when the hits of a search are shown (measured by the E2E test). */
export const SEARCH_SHOWN_MARK = 'pw:search-shown';

export const SEARCH_KINDS: readonly SearchKind[] = ['document', 'image', 'pdf', 'attachment', 'subnet'];

export interface SearchFilters {
  /** Empty: every kind. */
  kinds: readonly SearchKind[];
  tagIds: readonly string[];
  subnetId: string | null;
  /** Local dates `YYYY-MM-DD`, both inclusive; empty: open. */
  from: string;
  to: string;
  favoritesOnly: boolean;
  includeArchived: boolean;
  includeTrash: boolean;
}

export const NO_FILTERS: SearchFilters = {
  kinds: [],
  tagIds: [],
  subnetId: null,
  from: '',
  to: '',
  favoritesOnly: false,
  includeArchived: false,
  includeTrash: false,
};

export type SearchStatus = 'idle' | 'searching' | 'done' | 'error';

interface SearchStore {
  query: string;
  filters: SearchFilters;
  filtersVisible: boolean;
  status: SearchStatus;
  hits: SearchHit[];
  error: CommandError | null;
  /** Index of the selected hit, -1 for none. */
  selected: number;
  /** Counts focus requests (Ctrl+Shift+F), so the field can take the focus. */
  focusRequest: number;
  setQuery: (query: string) => void;
  setFilters: (change: Partial<SearchFilters>) => void;
  clearFilters: () => void;
  showFilters: (visible: boolean) => void;
  select: (index: number) => void;
  requestFocus: () => void;
  /** Runs the current search again (for example after the index changed). */
  refresh: () => Promise<void>;
  reset: () => void;
}

/** Whether a filter differs from the start. */
export function filtering(filters: SearchFilters): boolean {
  return (
    filters.kinds.length > 0 ||
    filters.tagIds.length > 0 ||
    filters.subnetId !== null ||
    filters.from !== '' ||
    filters.to !== '' ||
    filters.favoritesOnly ||
    filters.includeArchived ||
    filters.includeTrash
  );
}

/** Midnight of a local date as an ISO time in UTC; `days` moves it. */
export function localMidnight(date: string, days = 0): string | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!match) {
    return null;
  }
  const value = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]) + days);
  return Number.isNaN(value.getTime()) ? null : value.toISOString();
}

/** The request Rust gets for a query and filters. */
export function searchRequest(query: string, filters: SearchFilters): SearchRequest {
  const request: SearchRequest = { query, limit: SEARCH_LIMIT };
  if (filters.kinds.length > 0) {
    request.kinds = [...filters.kinds];
  }
  if (filters.tagIds.length > 0) {
    request.tagIds = [...filters.tagIds];
  }
  if (filters.subnetId !== null) {
    request.subnetId = filters.subnetId;
  }
  const from = localMidnight(filters.from);
  if (from) {
    request.updatedFrom = from;
  }
  // "bis" includes the whole day: before the next midnight.
  const before = localMidnight(filters.to, 1);
  if (before) {
    request.updatedBefore = before;
  }
  if (filters.favoritesOnly) {
    request.favoritesOnly = true;
  }
  if (filters.includeArchived) {
    request.includeArchived = true;
  }
  if (filters.includeTrash) {
    request.includeTrash = true;
  }
  return request;
}

let timer: ReturnType<typeof setTimeout> | null = null;
let sequence = 0;

export const useSearch = create<SearchStore>((set, get) => {
  const run = async () => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    const { query, filters } = get();
    const own = ++sequence;
    if (query.trim() === '') {
      set({ status: 'idle', hits: [], error: null, selected: -1 });
      return;
    }
    set({ status: 'searching' });
    const result = await call('search_query', searchRequest(query, filters));
    if (own !== sequence) {
      return;
    }
    if (result.ok) {
      set({ status: 'done', hits: result.value, error: null, selected: result.value.length > 0 ? 0 : -1 });
    } else {
      set({ status: 'error', hits: [], error: result.error, selected: -1 });
    }
  };
  const later = () => {
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void run();
    }, SEARCH_DELAY_MS);
  };
  return {
    query: '',
    filters: NO_FILTERS,
    filtersVisible: false,
    status: 'idle',
    hits: [],
    error: null,
    selected: -1,
    focusRequest: 0,
    setQuery: (query) => {
      set({ query });
      later();
    },
    setFilters: (change) => {
      set({ filters: { ...get().filters, ...change } });
      void run();
    },
    clearFilters: () => {
      set({ filters: NO_FILTERS });
      void run();
    },
    showFilters: (visible) => {
      set({ filtersVisible: visible });
    },
    select: (index) => {
      const { hits } = get();
      set({ selected: hits.length === 0 ? -1 : Math.max(0, Math.min(hits.length - 1, index)) });
    },
    requestFocus: () => {
      set({ focusRequest: get().focusRequest + 1 });
    },
    refresh: run,
    reset: () => {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      sequence += 1;
      set({
        query: '',
        filters: NO_FILTERS,
        filtersVisible: false,
        status: 'idle',
        hits: [],
        error: null,
        selected: -1,
      });
    },
  };
});
