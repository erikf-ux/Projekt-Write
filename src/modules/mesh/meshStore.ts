/**
 * The Mesh view (section 15, ADR 0022): which view is shown, what is open,
 * what is selected, and the undo steps of the Mesh's own actions
 * (connections, positions). Assigning to a subnet goes through the
 * structure and its undo (section 8).
 */
import { create } from 'zustand';
import type { MessageKey } from '@/i18n';
import { call } from '@/services/ipc';
import { useUi } from '@/modules/settings/uiStore';
import type { CommandError } from '@/types/generated/CommandError';
import type { MeshConnection } from '@/types/generated/MeshConnection';
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshPosition } from '@/types/generated/MeshPosition';
import type { MeshView } from '@/types/generated/MeshView';

/** The cluster of elements without a subnet (Rust: `UNASSIGNED_ID`). */
export const UNASSIGNED = 'unassigned';
/** Undo steps the Mesh keeps. */
const UNDO_LIMIT = 50;

export interface MeshPlace {
  view: MeshView;
  scopeId: string;
}

interface UndoStep {
  key: MessageKey;
  run: () => Promise<boolean>;
}

interface MeshStore {
  place: MeshPlace;
  /** Open clusters per view and scope (`view:scope`). */
  expanded: Record<string, readonly string[]>;
  depth: 1 | 2;
  graph: MeshGraph | null;
  loading: boolean;
  error: CommandError | null;
  selected: readonly string[];
  selectedEdge: string | null;
  /** Earlier places, for "Zurück". */
  trail: readonly MeshPlace[];
  undoSteps: readonly UndoStep[];
  /** Bumped when the content changed elsewhere (structure, documents). */
  generation: number;

  load: () => Promise<void>;
  show: (place: MeshPlace) => Promise<void>;
  back: () => Promise<void>;
  toggle: (subnetId: string) => Promise<void>;
  setDepth: (depth: 1 | 2) => Promise<void>;
  select: (ids: readonly string[]) => void;
  selectEdge: (id: string | null) => void;
  connect: (a: string, b: string) => Promise<boolean>;
  disconnect: (connectionId: string) => Promise<boolean>;
  savePositions: (
    positions: readonly MeshPosition[],
    before: readonly MeshPosition[],
    remove?: readonly string[],
  ) => Promise<void>;
  /** Automatic layout (ELK); `all` also moves pinned nodes. */
  autoLayout: (all: boolean) => Promise<boolean>;
  layouting: boolean;
  undo: () => Promise<boolean>;
  canUndo: () => boolean;
  invalidate: () => void;
}

export function placeKey(place: MeshPlace): string {
  return `${place.view}:${place.scopeId}`;
}

const GLOBAL: MeshPlace = { view: 'global', scopeId: '' };

export const useMesh = create<MeshStore>((set, get) => {
  const notifyError = (error: CommandError) => {
    useUi.getState().showNotice({ tone: 'error', key: 'mesh.failed', error });
  };

  const pushUndo = (step: UndoStep) => {
    set((state) => ({ undoSteps: [...state.undoSteps, step].slice(-UNDO_LIMIT) }));
  };

  let request = 0;

  return {
    place: GLOBAL,
    expanded: {},
    depth: 1,
    graph: null,
    loading: false,
    error: null,
    selected: [],
    selectedEdge: null,
    trail: [],
    undoSteps: [],
    generation: 0,

    load: async () => {
      const { place, expanded, depth } = get();
      const mine = ++request;
      set({ loading: true });
      const result = await call('mesh_graph', {
        view: place.view,
        ...(place.scopeId === '' ? {} : { scopeId: place.scopeId }),
        expanded: [...(expanded[placeKey(place)] ?? [])],
        ...(place.view === 'focus' ? { depth } : {}),
      });
      if (mine !== request) {
        return;
      }
      if (result.ok) {
        const ids = new Set(result.value.nodes.map((node) => node.id));
        set((state) => ({
          graph: result.value,
          loading: false,
          error: null,
          selected: state.selected.filter((id) => ids.has(id)),
          selectedEdge: result.value.edges.some((edge) => edge.id === state.selectedEdge)
            ? state.selectedEdge
            : null,
        }));
      } else {
        set({ loading: false, error: result.error });
      }
    },

    show: async (place) => {
      const current = get().place;
      if (placeKey(current) !== placeKey(place)) {
        set((state) => ({
          place,
          trail: [...state.trail, current].slice(-50),
          selected: [],
          selectedEdge: null,
        }));
      }
      await get().load();
    },

    back: async () => {
      const trail = get().trail;
      const previous = trail.at(-1);
      if (!previous) {
        return;
      }
      set({ place: previous, trail: trail.slice(0, -1), selected: [], selectedEdge: null });
      await get().load();
    },

    toggle: async (subnetId) => {
      const key = placeKey(get().place);
      set((state) => {
        const open = state.expanded[key] ?? [];
        const next = open.includes(subnetId) ? open.filter((id) => id !== subnetId) : [...open, subnetId];
        return { expanded: { ...state.expanded, [key]: next } };
      });
      await get().load();
    },

    setDepth: async (depth) => {
      set({ depth });
      if (get().place.view === 'focus') {
        await get().load();
      }
    },

    select: (ids) => {
      set({ selected: ids, selectedEdge: null });
    },

    selectEdge: (id) => {
      set({ selectedEdge: id, selected: [] });
    },

    connect: async (a, b) => {
      const result = await call('mesh_connect', { nodeA: a, nodeB: b });
      if (!result.ok) {
        notifyError(result.error);
        return false;
      }
      const made = result.value;
      pushUndo({
        key: 'mesh.undo.connect',
        run: async () => (await call('mesh_disconnect', { id: made.id })).ok,
      });
      useUi.getState().showNotice({ tone: 'info', key: 'mesh.connected' });
      await get().load();
      return true;
    },

    disconnect: async (connectionId) => {
      const result = await call('mesh_disconnect', { id: connectionId });
      if (!result.ok) {
        notifyError(result.error);
        return false;
      }
      const removed: MeshConnection = result.value;
      pushUndo({
        key: 'mesh.undo.disconnect',
        run: async () =>
          (
            await call('mesh_restore_connection', {
              id: removed.id,
              nodeA: removed.nodeA,
              nodeB: removed.nodeB,
              kind: removed.kind,
              ...(removed.note === null ? {} : { note: removed.note }),
            })
          ).ok,
      });
      set({ selectedEdge: null });
      useUi.getState().showNotice({ tone: 'info', key: 'mesh.disconnected' });
      await get().load();
      return true;
    },

    savePositions: async (positions, before, remove = []) => {
      const place = get().place;
      const scope = place.scopeId === '' ? {} : { scopeId: place.scopeId };
      const result = await call('mesh_save_positions', {
        view: place.view,
        ...scope,
        positions: [...positions],
        ...(remove.length > 0 ? { remove: [...remove] } : {}),
      });
      if (!result.ok) {
        notifyError(result.error);
        return;
      }
      // The view knows what is pinned now.
      void get().load();
      // Nodes without a stored position before: dropped again on undo.
      const had = new Set(before.map((position) => position.nodeId));
      const dropped = positions.map((position) => position.nodeId).filter((id) => !had.has(id));
      pushUndo({
        key: 'mesh.undo.move',
        run: async () => {
          const back = await call('mesh_save_positions', {
            view: place.view,
            ...scope,
            positions: [...before],
            ...(dropped.length > 0 ? { remove: dropped } : {}),
          });
          return back.ok;
        },
      });
    },

    layouting: false,

    autoLayout: async (all) => {
      if (get().layouting) {
        return false;
      }
      set({ layouting: true });
      try {
        // What is pinned now, not what was drawn last.
        await get().load();
        const graph = get().graph;
        if (!graph) {
          return false;
        }
        const { elkPositions, layoutChanges } = await import('./autoLayout');
        const positions = await elkPositions(graph);
        const changes = layoutChanges(graph, positions, all);
        if (changes.positions.length > 0) {
          await get().savePositions(changes.positions, changes.before);
          // The undo step of savePositions is the layout's.
          set((state) => {
            const steps = [...state.undoSteps];
            const last = steps.pop();
            return last ? { undoSteps: [...steps, { ...last, key: 'mesh.undo.layout' }] } : {};
          });
        }
        useUi.getState().showNotice({ tone: 'info', key: all ? 'mesh.layoutAllDone' : 'mesh.layoutDone' });
        await get().load();
        return true;
      } catch {
        useUi.getState().showNotice({ tone: 'error', key: 'mesh.layoutFailed' });
        return false;
      } finally {
        set({ layouting: false });
      }
    },

    undo: async () => {
      const step = get().undoSteps.at(-1);
      if (!step) {
        return false;
      }
      set((state) => ({ undoSteps: state.undoSteps.slice(0, -1) }));
      const done = await step.run();
      if (done) {
        useUi.getState().showNotice({ tone: 'info', key: step.key });
      }
      await get().load();
      return done;
    },

    canUndo: () => get().undoSteps.length > 0,

    invalidate: () => {
      set((state) => ({ generation: state.generation + 1 }));
    },
  };
});
