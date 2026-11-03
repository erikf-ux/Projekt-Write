/**
 * Library commands (Phase 2). Folders are chosen in Rust; the frontend only
 * passes the tokens it received back (section 8).
 */
import type { LocationPurpose } from '@/types/generated/LocationPurpose';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import { call, callWithProgress } from './ipc';

export const libraryService = {
  status: () => call('library_status', {}),
  defaultLocation: () => call('library_default_location', {}),
  chooseLocation: (purpose: LocationPurpose, title: string) =>
    call('library_choose_location', { purpose, title }),
  create: (location: string, acceptWarning: boolean) => call('library_create', { location, acceptWarning }),
  open: (location: string, acceptMoved: boolean) => call('library_open', { location, acceptMoved }),
  reopen: () => call('library_reopen', {}),
  move: (location: string, acceptWarning: boolean, onProgress: (progress: MoveProgress) => void) =>
    callWithProgress('library_move', { location, acceptWarning }, onProgress),
  cancelMove: () => call('library_cancel_move', {}),
  acceptLocation: () => call('library_accept_location', {}),
};
