/**
 * What the Explorer shows besides the data: open entries and subnets, the
 * filter (ADR 0017, decision 13), and requests to show a subnet or a
 * document from elsewhere (breadcrumbs, palette).
 */
import { create } from 'zustand';
import { CLOSED_AT_START, NO_FILTER, type ExplorerFilter, type ExplorerGroup } from './explorerRows';

interface ExplorerStore {
  collapsed: ReadonlySet<ExplorerGroup>;
  expanded: ReadonlySet<string>;
  filter: ExplorerFilter;
  filterVisible: boolean;
  /** A row the Explorer should select and focus (`nonce` repeats a request). */
  reveal: { key: string; nonce: number } | null;
  toggleGroup: (group: ExplorerGroup, open?: boolean) => void;
  toggleSubnet: (id: string, open?: boolean) => void;
  /** Opens every subnet of a path (top level first). */
  expandPath: (ids: readonly string[]) => void;
  setFilterText: (text: string) => void;
  toggleFilterTag: (id: string) => void;
  clearFilter: () => void;
  showFilter: (visible: boolean) => void;
  /** Selects the row `key`, opening what is needed (keys as in `explorerRows`). */
  revealRow: (key: string, path?: readonly string[]) => void;
  reset: () => void;
}

export const useExplorer = create<ExplorerStore>((set, get) => ({
  collapsed: new Set(CLOSED_AT_START),
  expanded: new Set(),
  filter: NO_FILTER,
  filterVisible: false,
  reveal: null,

  toggleGroup: (group, open) => {
    set((state) => {
      const next = new Set(state.collapsed);
      if (open ?? next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return { collapsed: next };
    });
  },

  toggleSubnet: (id, open) => {
    set((state) => {
      const next = new Set(state.expanded);
      if (open ?? !next.has(id)) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return { expanded: next };
    });
  },

  expandPath: (ids) => {
    if (ids.every((id) => get().expanded.has(id))) {
      return;
    }
    set((state) => ({ expanded: new Set([...state.expanded, ...ids]) }));
  },

  setFilterText: (text) => {
    set((state) => ({ filter: { ...state.filter, text } }));
  },

  toggleFilterTag: (id) => {
    set((state) => {
      const tagIds = state.filter.tagIds.includes(id)
        ? state.filter.tagIds.filter((tag) => tag !== id)
        : [...state.filter.tagIds, id];
      return { filter: { ...state.filter, tagIds } };
    });
  },

  clearFilter: () => {
    set({ filter: NO_FILTER });
  },

  showFilter: (visible) => {
    set(visible ? { filterVisible: true } : { filterVisible: false, filter: NO_FILTER });
  },

  revealRow: (key, path = []) => {
    get().expandPath(path);
    if (key.startsWith('unassigned:')) {
      get().toggleGroup('unassigned', true);
    }
    set((state) => ({ reveal: { key, nonce: (state.reveal?.nonce ?? 0) + 1 } }));
  },

  reset: () => {
    set({
      collapsed: new Set(CLOSED_AT_START),
      expanded: new Set(),
      filter: NO_FILTER,
      filterVisible: false,
      reveal: null,
    });
  },
}));
