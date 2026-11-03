/**
 * The only module that talks to Tauri (enforced by ESLint).
 *
 * * Commands are typed: request and response types are generated from Rust
 *   (`src/types/generated`, ts-rs). Nothing is maintained twice.
 * * Every failure arrives as a `CommandError` (`{ code, messageKey, details? }`).
 *   Rejections that do not come from a command – for example a call the
 *   capability does not allow – are normalised to `IPC_REJECTED`; their raw
 *   text is discarded because it may contain internals.
 */
import { Channel, invoke } from '@tauri-apps/api/core';
import type { ChooseLocationRequest } from '@/types/generated/ChooseLocationRequest';
import type { CommandError } from '@/types/generated/CommandError';
import type { CreateLibraryRequest } from '@/types/generated/CreateLibraryRequest';
import type { EmptyRequest } from '@/types/generated/EmptyRequest';
import type { ErrorCode } from '@/types/generated/ErrorCode';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { LocationChoice } from '@/types/generated/LocationChoice';
import type { MoveLibraryRequest } from '@/types/generated/MoveLibraryRequest';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import type { OpenLibraryRequest } from '@/types/generated/OpenLibraryRequest';
import type { SystemCheckRequest } from '@/types/generated/SystemCheckRequest';
import type { SystemCheckResponse } from '@/types/generated/SystemCheckResponse';

/** Every app command with its request and response type. */
export interface Commands {
  system_check: { request: SystemCheckRequest; response: SystemCheckResponse };
  library_status: { request: EmptyRequest; response: LibraryStatus };
  library_default_location: { request: EmptyRequest; response: LocationChoice | null };
  library_choose_location: { request: ChooseLocationRequest; response: LocationChoice | null };
  library_create: { request: CreateLibraryRequest; response: LibraryStatus };
  library_open: { request: OpenLibraryRequest; response: LibraryStatus };
  library_reopen: { request: EmptyRequest; response: LibraryStatus };
  library_move: { request: MoveLibraryRequest; response: LibraryStatus };
  library_cancel_move: { request: EmptyRequest; response: boolean };
  library_accept_location: { request: EmptyRequest; response: LibraryStatus };
}

/** Commands that report progress through a channel argument `onProgress`. */
export interface ProgressCommands {
  library_move: MoveProgress;
}

export type CommandName = keyof Commands;

export type CommandResult<T> = { ok: true; value: T } | { ok: false; error: CommandError };

const errorCodes: Record<ErrorCode, true> = {
  INVALID_INPUT: true,
  DATABASE: true,
  SCHEMA_TOO_NEW: true,
  INTERNAL: true,
  IPC_REJECTED: true,
  FILE_SYSTEM: true,
  LIBRARY_NOT_OPEN: true,
  LIBRARY_NOT_FOUND: true,
  LIBRARY_INVALID: true,
  LIBRARY_TOO_NEW: true,
  LIBRARY_LOCKED: true,
  LIBRARY_LOCK_UNSUPPORTED: true,
  LIBRARY_MOVED: true,
  LIBRARY_UNSAFE_PATH: true,
  LIBRARY_DATABASE_MISSING: true,
  LIBRARY_DATABASE_DAMAGED: true,
  LIBRARY_BACKUP_FAILED: true,
  LIBRARY_BUSY: true,
  FOLDER_NOT_EMPTY: true,
  FOLDER_IS_LIBRARY: true,
  LOCATION_NOT_ACCEPTED: true,
  LOCATION_EXPIRED: true,
  PATH_NOT_UNICODE: true,
  MOVE_CANCELLED: true,
  MOVE_TARGET_INVALID: true,
  MOVE_VERIFICATION_FAILED: true,
  MOVE_SWITCH_FAILED: true,
};

export const IPC_REJECTED: CommandError = { code: 'IPC_REJECTED', messageKey: 'error.ipcRejected' };

export function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  const { code, messageKey, details } = candidate;
  return (
    typeof code === 'string' &&
    Object.hasOwn(errorCodes, code) &&
    typeof messageKey === 'string' &&
    (details === undefined || (typeof details === 'object' && details !== null))
  );
}

export function normalizeError(raw: unknown): CommandError {
  return isCommandError(raw) ? raw : IPC_REJECTED;
}

/** Calls an app command. Never throws. */
export async function call<C extends CommandName>(
  command: C,
  request: Commands[C]['request'],
): Promise<CommandResult<Commands[C]['response']>> {
  try {
    const value = await invoke<Commands[C]['response']>(command, { request });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}

/**
 * Calls a command that streams progress. Progress messages arrive through a
 * Tauri channel (no event permission needed). Never throws.
 */
export async function callWithProgress<C extends CommandName & keyof ProgressCommands>(
  command: C,
  request: Commands[C]['request'],
  onProgress: (progress: ProgressCommands[C]) => void,
): Promise<CommandResult<Commands[C]['response']>> {
  const channel = new Channel<ProgressCommands[C]>();
  channel.onmessage = onProgress;
  try {
    const value = await invoke<Commands[C]['response']>(command, { request, onProgress: channel });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}
