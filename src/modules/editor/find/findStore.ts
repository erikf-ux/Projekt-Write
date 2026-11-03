/**
 * Find and replace in the document (section 12): one search per tab, kept
 * while the tab is open. The find bar shows it; the editor session runs it.
 */
import { create } from 'zustand';

export interface FindState {
  /** With the replace field. */
  replace: boolean;
  term: string;
  replacement: string;
  caseSensitive: boolean;
  wholeWord: boolean;
  regex: boolean;
  /** Counts focus requests, so the bar can focus its field again. */
  focusRequest: number;
}

const INITIAL: Omit<FindState, 'focusRequest'> = {
  replace: false,
  term: '',
  replacement: '',
  caseSensitive: false,
  wholeWord: false,
  regex: false,
};

interface FindStore {
  /** Open find bars by document. */
  bars: Record<string, FindState>;
  open: (id: string, replace: boolean, term?: string) => void;
  change: (id: string, change: Partial<Omit<FindState, 'focusRequest'>>) => void;
  close: (id: string) => void;
}

export const useFind = create<FindStore>((set) => ({
  bars: {},
  open: (id, replace, term) => {
    set((state) => {
      const current = state.bars[id] ?? { ...INITIAL, focusRequest: 0 };
      return {
        bars: {
          ...state.bars,
          [id]: {
            ...current,
            replace: replace || current.replace,
            term: term ?? current.term,
            focusRequest: current.focusRequest + 1,
          },
        },
      };
    });
  },
  change: (id, change) => {
    set((state) => {
      const current = state.bars[id];
      return current ? { bars: { ...state.bars, [id]: { ...current, ...change } } } : {};
    });
  },
  close: (id) => {
    set((state) => ({ bars: Object.fromEntries(Object.entries(state.bars).filter(([key]) => key !== id)) }));
  },
}));
