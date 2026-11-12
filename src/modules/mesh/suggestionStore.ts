/**
 * Suggestions of the grouping analysis (section 15): run in the
 * background, cancellable; the user decides: Annehmen, Ablehnen, Später.
 * "Später" only hides a suggestion until the list is loaded again.
 */
import { create } from 'zustand';
import { useUi } from '@/modules/settings/uiStore';
import { call, callWithProgress } from '@/services/ipc';
import type { AnalysisProgress } from '@/types/generated/AnalysisProgress';
import type { MeshSuggestion } from '@/types/generated/MeshSuggestion';
import { useMesh } from './meshStore';

interface SuggestionStore {
  open: boolean;
  suggestions: readonly MeshSuggestion[];
  threshold: number;
  later: readonly string[];
  progress: AnalysisProgress | null;
  running: boolean;
  toggle: () => void;
  load: () => Promise<void>;
  analyze: () => Promise<void>;
  cancel: () => Promise<void>;
  decide: (id: string, decision: 'accept' | 'reject') => Promise<boolean>;
  postpone: (id: string) => void;
  setThreshold: (value: number) => Promise<void>;
}

export const useSuggestions = create<SuggestionStore>((set, get) => ({
  open: false,
  suggestions: [],
  threshold: 0.35,
  later: [],
  progress: null,
  running: false,

  toggle: () => {
    const open = !get().open;
    set({ open });
    if (open) {
      void get().load();
    }
  },

  load: async () => {
    const result = await call('mesh_suggestions', { limit: 200 });
    if (result.ok) {
      set({ suggestions: result.value.suggestions, threshold: result.value.threshold, later: [] });
    }
  },

  analyze: async () => {
    if (get().running) {
      return;
    }
    set({ running: true, progress: null });
    const result = await callWithProgress('mesh_analyze', {}, (progress) => {
      set({ progress });
    });
    set({ running: false, progress: null });
    if (result.ok) {
      useUi
        .getState()
        .showNotice({ tone: 'info', key: 'mesh.analysisDone', params: { count: result.value.created } });
      await get().load();
    } else if (result.error.code === 'MESH_ANALYSIS_CANCELLED') {
      useUi.getState().showNotice({ tone: 'info', key: 'mesh.analysisCancelled' });
    } else {
      useUi.getState().showNotice({ tone: 'error', key: 'mesh.failed', error: result.error });
    }
  },

  cancel: async () => {
    await call('mesh_analysis_cancel', {});
  },

  decide: async (id, decision) => {
    const result = await call('mesh_suggestion_decide', { id, decision });
    if (!result.ok) {
      useUi.getState().showNotice({ tone: 'error', key: 'mesh.failed', error: result.error });
      await get().load();
      return false;
    }
    set((state) => ({ suggestions: state.suggestions.filter((suggestion) => suggestion.id !== id) }));
    useUi
      .getState()
      .showNotice({ tone: 'info', key: decision === 'accept' ? 'mesh.accepted' : 'mesh.rejected' });
    if (decision === 'accept') {
      await useMesh.getState().load();
    }
    return true;
  },

  postpone: (id) => {
    set((state) => ({ later: [...state.later, id] }));
  },

  setThreshold: async (value) => {
    const result = await call('mesh_set_threshold', { threshold: value });
    if (result.ok) {
      set({ threshold: result.value });
    }
  },
}));
