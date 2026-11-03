/**
 * The writing statistics of open documents, for the status bar and the
 * statistics dialog. Editor sessions write them shortly after a change of
 * text or selection (`session.ts`).
 */
import { create } from 'zustand';
import type { TextStatistics } from './statistics';

export interface DocumentStatistics {
  document: TextStatistics;
  /** The selection, if text is selected. */
  selection: TextStatistics | null;
}

interface StatisticsStore {
  byDocument: Record<string, DocumentStatistics>;
  /** The statistics dialog is open. */
  dialogOpen: boolean;
  set: (id: string, statistics: DocumentStatistics) => void;
  remove: (id: string) => void;
  showDialog: (open: boolean) => void;
}

export const useStatistics = create<StatisticsStore>((set) => ({
  byDocument: {},
  dialogOpen: false,
  set: (id, statistics) => {
    set((state) => ({ byDocument: { ...state.byDocument, [id]: statistics } }));
  },
  remove: (id) => {
    set((state) => ({
      byDocument: Object.fromEntries(Object.entries(state.byDocument).filter(([key]) => key !== id)),
    }));
  },
  showDialog: (open) => {
    set({ dialogOpen: open });
  },
}));
