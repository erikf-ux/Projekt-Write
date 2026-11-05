/**
 * Which structure dialog is open: a subnet to create or edit, the subnet
 * picker for moving and assigning, the tags of a document, the tag
 * manager (ADR 0017, decision 14).
 */
import { create } from 'zustand';

export type SubnetDialogState = { mode: 'create'; parentId: string | null } | { mode: 'edit'; id: string };

export type PickerState =
  { mode: 'moveDocument' | 'assignDocument'; documentId: string } | { mode: 'moveSubnet'; subnetId: string };

interface StructureDialogs {
  subnet: SubnetDialogState | null;
  picker: PickerState | null;
  /** Document whose tags are edited. */
  tags: string | null;
  tagManager: boolean;
  openSubnet: (state: SubnetDialogState) => void;
  openPicker: (state: PickerState) => void;
  openTags: (documentId: string) => void;
  openTagManager: () => void;
  close: () => void;
}

export const useStructureDialogs = create<StructureDialogs>((set) => ({
  subnet: null,
  picker: null,
  tags: null,
  tagManager: false,
  openSubnet: (subnet) => {
    set({ subnet, picker: null, tags: null, tagManager: false });
  },
  openPicker: (picker) => {
    set({ picker, subnet: null, tags: null, tagManager: false });
  },
  openTags: (tags) => {
    set({ tags, subnet: null, picker: null, tagManager: false });
  },
  openTagManager: () => {
    set({ tagManager: true, subnet: null, picker: null, tags: null });
  },
  close: () => {
    set({ subnet: null, picker: null, tags: null, tagManager: false });
  },
}));
