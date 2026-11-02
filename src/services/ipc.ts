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
import { invoke } from '@tauri-apps/api/core';
import type { CommandError } from '@/types/generated/CommandError';
import type { ErrorCode } from '@/types/generated/ErrorCode';
import type { SystemCheckRequest } from '@/types/generated/SystemCheckRequest';
import type { SystemCheckResponse } from '@/types/generated/SystemCheckResponse';

/** Every app command with its request and response type. */
export interface Commands {
  system_check: { request: SystemCheckRequest; response: SystemCheckResponse };
}

export type CommandName = keyof Commands;

export type CommandResult<T> = { ok: true; value: T } | { ok: false; error: CommandError };

const errorCodes: Record<ErrorCode, true> = {
  INVALID_INPUT: true,
  DATABASE: true,
  SCHEMA_TOO_NEW: true,
  INTERNAL: true,
  IPC_REJECTED: true,
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
