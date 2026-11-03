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
import type { CloseResponseRequest } from '@/types/generated/CloseResponseRequest';
import type { CommandError } from '@/types/generated/CommandError';
import type { CreateDocumentRequest } from '@/types/generated/CreateDocumentRequest';
import type { CreateLibraryRequest } from '@/types/generated/CreateLibraryRequest';
import type { DocumentRequest } from '@/types/generated/DocumentRequest';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { DocumentText } from '@/types/generated/DocumentText';
import type { EmptyRequest } from '@/types/generated/EmptyRequest';
import type { ErrorCode } from '@/types/generated/ErrorCode';
import type { FocusModeRequest } from '@/types/generated/FocusModeRequest';
import type { ImportDocumentRequest } from '@/types/generated/ImportDocumentRequest';
import type { ImportResult } from '@/types/generated/ImportResult';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { LocationChoice } from '@/types/generated/LocationChoice';
import type { MenuStateRequest } from '@/types/generated/MenuStateRequest';
import type { MoveLibraryRequest } from '@/types/generated/MoveLibraryRequest';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import type { OpenLibraryRequest } from '@/types/generated/OpenLibraryRequest';
import type { RenameDocumentRequest } from '@/types/generated/RenameDocumentRequest';
import type { SaveDocumentRequest } from '@/types/generated/SaveDocumentRequest';
import type { SystemCheckRequest } from '@/types/generated/SystemCheckRequest';
import type { SystemCheckResponse } from '@/types/generated/SystemCheckResponse';
import type { UiEvent } from '@/types/generated/UiEvent';
import type { UiState } from '@/types/generated/UiState';
import type { UpdateUiSettingsRequest } from '@/types/generated/UpdateUiSettingsRequest';

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
  ui_state: { request: EmptyRequest; response: UiState };
  ui_update_settings: { request: UpdateUiSettingsRequest; response: UiState };
  ui_subscribe: { request: EmptyRequest; response: null };
  ui_menu_state: { request: MenuStateRequest; response: null };
  ui_set_focus_mode: { request: FocusModeRequest; response: null };
  ui_close_response: { request: CloseResponseRequest; response: null };
  documents_list: { request: EmptyRequest; response: DocumentSummary[] };
  document_create: { request: CreateDocumentRequest; response: DocumentSummary };
  document_read: { request: DocumentRequest; response: DocumentText };
  document_save: { request: SaveDocumentRequest; response: DocumentSummary };
  document_rename: { request: RenameDocumentRequest; response: DocumentSummary };
  document_import: { request: ImportDocumentRequest; response: ImportResult };
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
  SETTINGS_READ_ONLY: true,
  SETTINGS_NOT_SAVED: true,
  DOCUMENT_NOT_FOUND: true,
  DOCUMENT_TOO_LARGE: true,
  DOCUMENT_NOT_TEXT: true,
  DOCUMENT_FORMAT_UNSUPPORTED: true,
  IMPORT_UNSUPPORTED_TYPE: true,
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

/**
 * Registers the handler for messages from Rust: menu clicks and close
 * requests of the window. A later call replaces the handler. Never throws.
 */
export async function subscribeUiEvents(onEvent: (event: UiEvent) => void): Promise<CommandResult<null>> {
  const channel = new Channel<UiEvent>();
  channel.onmessage = onEvent;
  try {
    const value = await invoke<null>('ui_subscribe', { request: {}, onEvent: channel });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}
