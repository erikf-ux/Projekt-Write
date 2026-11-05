/**
 * Subnets, memberships and tags in the interface (Phase 7, ADR 0017).
 *
 * Rust holds the data and the undo journal; every action answers with all
 * subnets and tags and the structure of the documents involved, which the
 * store takes over as it is. After each step a notice offers "Rückgängig"
 * (section 8: structural actions have an undo of their own, apart from
 * the editor's).
 */
import { create } from 'zustand';
import { ask } from '@/components/prompt/promptStore';
import { translate, useLocale, type MessageKey } from '@/i18n';
import { useUi } from '@/modules/settings/uiStore';
import { call, type CommandResult } from '@/services/ipc';
import type { CommandError } from '@/types/generated/CommandError';
import type { CreateSubnetRequest } from '@/types/generated/CreateSubnetRequest';
import type { StructureAction } from '@/types/generated/StructureAction';
import type { StructureUpdate } from '@/types/generated/StructureUpdate';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';
import type { TagSummary } from '@/types/generated/TagSummary';
import type { UndoHistory } from '@/types/generated/UndoHistory';
import type { UndoStep } from '@/types/generated/UndoStep';
import type { UpdateSubnetRequest } from '@/types/generated/UpdateSubnetRequest';
import { buildIndex, subtree } from './tree';

/** Subnets (primary first) and tags of a document. */
export interface Placement {
  subnetIds: readonly string[];
  tagIds: readonly string[];
}

export const NO_PLACEMENT: Placement = { subnetIds: [], tagIds: [] };

/** Steps that change trash or archive: the document lists reload after them. */
const DOCUMENT_STATE_ACTIONS: readonly StructureAction[] = [
  'documentDelete',
  'documentRestore',
  'documentArchive',
  'documentUnarchive',
];

interface StructureStore {
  loaded: boolean;
  subnets: SubnetSummary[];
  tags: TagSummary[];
  byDocument: Record<string, Placement>;
  history: UndoHistory;
  load: () => Promise<void>;
  reset: () => void;
  /** Takes over what Rust answered. */
  apply: (update: StructureUpdate) => void;
  refreshDocument: (id: string) => Promise<void>;
  refreshHistory: () => Promise<void>;
  createSubnet: (request: CreateSubnetRequest) => Promise<{ id: string | null; error: CommandError | null }>;
  updateSubnet: (request: UpdateSubnetRequest) => Promise<CommandError | null>;
  moveSubnet: (id: string, parentId: string | null) => Promise<CommandError | null>;
  /** Asks first; elements are never deleted. */
  deleteSubnet: (id: string) => Promise<CommandError | null>;
  assign: (documentId: string, subnetId: string) => Promise<CommandError | null>;
  unassign: (documentId: string, subnetId: string) => Promise<CommandError | null>;
  setPrimary: (documentId: string, subnetId: string) => Promise<CommandError | null>;
  /** "Verschieben nach …"; `null` takes it out of every subnet. */
  moveDocument: (documentId: string, subnetId: string | null) => Promise<CommandError | null>;
  createTag: (name: string) => Promise<CommandError | null>;
  /** A name another tag has: asks whether to merge. */
  renameTag: (id: string, name: string) => Promise<CommandError | null>;
  mergeTags: (sourceId: string, targetId: string) => Promise<CommandError | null>;
  deleteTag: (id: string) => Promise<CommandError | null>;
  tagDocument: (documentId: string, name: string) => Promise<CommandError | null>;
  untagDocument: (documentId: string, tagId: string) => Promise<CommandError | null>;
  undo: () => Promise<CommandError | null>;
  redo: () => Promise<CommandError | null>;
  /** Another module did a step (trash, archive): notice with "Rückgängig". */
  noteStep: () => Promise<void>;
  /** A document removed for good. */
  forget: (id: string) => void;
}

function t(key: MessageKey, params?: Record<string, string | number>): string {
  return translate(useLocale.getState().locale, key, params);
}

/** "„Kapitel“ verschoben" – what a step did, for notices and the palette. */
export function describeStep(step: UndoStep): string {
  return t(`structure.action.${step.action}`, { subject: step.subject });
}

function showDone(step: UndoStep | null, key: MessageKey = 'structure.done') {
  if (!step) {
    return;
  }
  useUi.getState().showNotice({
    tone: 'info',
    key,
    params: { what: describeStep(step) },
    action:
      key === 'structure.undone'
        ? { labelKey: 'structure.redo', run: () => void useStructure.getState().redo() }
        : { labelKey: 'structure.undo', run: () => void useStructure.getState().undo() },
  });
}

function showError(error: CommandError) {
  useUi.getState().showNotice({ tone: 'error', key: 'structure.failed', error });
}

/** Document lists and the trash reload when a step changed them. */
let reloadDocuments: () => Promise<void> = () => Promise.resolve();

/** Set by the workspace (it owns the document lists). */
export function setDocumentReloader(reload: () => Promise<void>): void {
  reloadDocuments = reload;
}

export const useStructure = create<StructureStore>((set, get) => {
  /** Runs a changing command and takes over its answer. */
  const step = async (result: Promise<CommandResult<StructureUpdate>>, notify = true) => {
    const answer = await result;
    if (!answer.ok) {
      return answer.error;
    }
    get().apply(answer.value);
    if (notify) {
      showDone(answer.value.step);
    }
    return null;
  };

  const replay = async (undo: boolean): Promise<CommandError | null> => {
    const result = await call(undo ? 'structure_undo' : 'structure_redo', {});
    if (!result.ok) {
      showError(result.error);
      // The journal is empty now (STRUCTURE_CHANGED): reload everything.
      await get().load();
      return result.error;
    }
    const update = result.value;
    if (!update) {
      useUi
        .getState()
        .showNotice({ tone: 'info', key: undo ? 'structure.nothingToUndo' : 'structure.nothingToRedo' });
      return null;
    }
    get().apply(update);
    if (update.step && DOCUMENT_STATE_ACTIONS.includes(update.step.action)) {
      await reloadDocuments();
    }
    showDone(update.step, undo ? 'structure.undone' : 'structure.redone');
    return null;
  };

  return {
    loaded: false,
    subnets: [],
    tags: [],
    byDocument: {},
    history: { undo: null, redo: null },

    load: async () => {
      const result = await call('structure_state', {});
      if (!result.ok) {
        set({ loaded: true });
        return;
      }
      const byDocument: Record<string, Placement> = {};
      for (const entry of result.value.documents) {
        byDocument[entry.id] = { subnetIds: entry.subnetIds, tagIds: entry.tagIds };
      }
      set({
        loaded: true,
        subnets: result.value.subnets,
        tags: result.value.tags,
        byDocument,
        history: result.value.history,
      });
    },

    reset: () => {
      set({ loaded: false, subnets: [], tags: [], byDocument: {}, history: { undo: null, redo: null } });
    },

    apply: (update) => {
      set((state) => {
        const byDocument = { ...state.byDocument };
        for (const entry of update.documents) {
          // An entry without subnets and tags is the same as none.
          byDocument[entry.id] = { subnetIds: entry.subnetIds, tagIds: entry.tagIds };
        }
        return { subnets: update.subnets, tags: update.tags, byDocument, history: update.history };
      });
    },

    refreshDocument: async (id) => {
      const result = await call('structure_document', { id });
      if (result.ok) {
        set((state) => ({
          byDocument: {
            ...state.byDocument,
            [id]: { subnetIds: result.value.subnetIds, tagIds: result.value.tagIds },
          },
        }));
      }
    },

    refreshHistory: async () => {
      const result = await call('structure_history', {});
      if (result.ok) {
        set({ history: result.value });
      }
    },

    createSubnet: async (request) => {
      const result = await call('subnet_create', request);
      if (!result.ok) {
        return { id: null, error: result.error };
      }
      get().apply(result.value);
      showDone(result.value.step);
      return { id: result.value.subjectId, error: null };
    },

    updateSubnet: (request) => step(call('subnet_update', request)),

    moveSubnet: (id, parentId) => step(call('subnet_move', { id, parentId })),

    deleteSubnet: async (id) => {
      const { subnets, byDocument } = get();
      const subnet = subnets.find((entry) => entry.id === id);
      if (!subnet) {
        return null;
      }
      const index = buildIndex(subnets, useLocale.getState().locale);
      const children = index.children.get(id)?.length ?? 0;
      const all = subtree(index, id);
      const direct = Object.values(byDocument).filter((placement) => placement.subnetIds.includes(id)).length;
      const below = Object.values(byDocument).filter((placement) =>
        placement.subnetIds.some((subnetId) => all.has(subnetId)),
      ).length;
      const answer = await ask({
        titleKey: 'subnet.deleteTitle',
        textKey: children > 0 ? 'subnet.deleteTextChildren' : 'subnet.deleteText',
        params: { name: subnet.name, count: direct, children, below },
        choices:
          children > 0
            ? [
                { id: 'liftUp', labelKey: 'subnet.deleteLiftUp', tone: 'primary' },
                { id: 'withChildren', labelKey: 'subnet.deleteWithChildren', tone: 'danger' },
                { id: 'cancel', labelKey: 'subnet.cancel' },
              ]
            : [
                { id: 'liftUp', labelKey: 'subnet.delete', tone: 'danger' },
                { id: 'cancel', labelKey: 'subnet.cancel' },
              ],
        cancel: 'cancel',
        testId: 'subnet-delete-prompt',
      });
      if (answer === 'cancel') {
        return null;
      }
      return step(call('subnet_delete', { id, withChildren: answer === 'withChildren' }));
    },

    assign: (documentId, subnetId) => step(call('document_assign', { id: documentId, subnetId })),
    unassign: (documentId, subnetId) => step(call('document_unassign', { id: documentId, subnetId })),
    setPrimary: (documentId, subnetId) => step(call('document_set_primary', { id: documentId, subnetId })),
    moveDocument: (documentId, subnetId) => step(call('document_move', { id: documentId, subnetId })),

    createTag: (name) => step(call('tag_create', { name })),

    renameTag: async (id, name) => {
      const result = await call('tag_rename', { id, name });
      if (result.ok) {
        get().apply(result.value);
        showDone(result.value.step);
        return null;
      }
      const existing = result.error.details?.['existing'];
      if (result.error.code !== 'TAG_NAME_TAKEN' || !existing) {
        return result.error;
      }
      const source = get().tags.find((tag) => tag.id === id);
      const target = get().tags.find((tag) => tag.id === existing);
      const answer = await ask({
        titleKey: 'tag.mergeTitle',
        textKey: 'tag.mergeText',
        params: { source: source?.name ?? name, target: target?.name ?? name },
        choices: [
          { id: 'merge', labelKey: 'tag.merge', tone: 'primary' },
          { id: 'cancel', labelKey: 'tag.cancel' },
        ],
        cancel: 'cancel',
        testId: 'tag-merge-prompt',
      });
      return answer === 'merge' ? get().mergeTags(id, existing) : null;
    },

    mergeTags: (sourceId, targetId) => step(call('tag_merge', { sourceId, targetId })),

    deleteTag: async (id) => {
      const tag = get().tags.find((entry) => entry.id === id);
      if (!tag) {
        return null;
      }
      const count = Object.values(get().byDocument).filter((placement) =>
        placement.tagIds.includes(id),
      ).length;
      const answer = await ask({
        titleKey: 'tag.deleteTitle',
        textKey: 'tag.deleteText',
        params: { name: tag.name, count },
        choices: [
          { id: 'delete', labelKey: 'tag.delete', tone: 'danger' },
          { id: 'cancel', labelKey: 'tag.cancel' },
        ],
        cancel: 'cancel',
        testId: 'tag-delete-prompt',
      });
      return answer === 'delete' ? step(call('tag_delete', { id })) : null;
    },

    tagDocument: (documentId, name) => step(call('document_tag', { id: documentId, name }), false),
    untagDocument: (documentId, tagId) => step(call('document_untag', { id: documentId, tagId }), false),

    undo: () => replay(true),
    redo: () => replay(false),

    noteStep: async () => {
      await get().refreshHistory();
      showDone(get().history.undo);
    },

    forget: (id) => {
      set((state) => {
        if (!(id in state.byDocument)) {
          return {};
        }
        return { byDocument: { ...state.byDocument, [id]: NO_PLACEMENT } };
      });
    },
  };
});

/** The subnets and tags of a document. */
export function placementOf(byDocument: Record<string, Placement>, id: string): Placement {
  return byDocument[id] ?? NO_PLACEMENT;
}
