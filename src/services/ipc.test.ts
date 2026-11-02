import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { call, IPC_REJECTED, isCommandError, normalizeError } from './ipc';

afterEach(() => {
  clearMocks();
});

const response = {
  appVersion: '0.1.0',
  platform: 'linux',
  sqliteVersion: '3.50.0',
  schemaVersion: 1,
  results: [{ check: 'database', passed: true }],
};

describe('call', () => {
  it('passes the request under the "request" key and returns the typed value', async () => {
    let received: unknown;
    mockIPC((command, args) => {
      received = { command, args };
      return response;
    });
    const result = await call('system_check', { checks: ['database'] });
    expect(received).toEqual({ command: 'system_check', args: { request: { checks: ['database'] } } });
    expect(result).toEqual({ ok: true, value: response });
  });

  it('returns command errors unchanged', async () => {
    const error = { code: 'INVALID_INPUT', messageKey: 'error.invalidInput', details: { field: 'checks' } };
    mockIPC(() => {
      throw error;
    });
    expect(await call('system_check', { checks: [] })).toEqual({ ok: false, error });
  });

  it('normalises rejections that do not come from a command', async () => {
    mockIPC(() => {
      throw 'Command system_check not allowed by ACL';
    });
    expect(await call('system_check', { checks: ['database'] })).toEqual({ ok: false, error: IPC_REJECTED });
  });
});

describe('isCommandError', () => {
  it('accepts the uniform format', () => {
    expect(isCommandError({ code: 'DATABASE', messageKey: 'error.database' })).toBe(true);
    expect(isCommandError({ code: 'DATABASE', messageKey: 'error.database', details: {} })).toBe(true);
  });

  it('rejects everything else', () => {
    for (const value of [
      null,
      'error',
      42,
      { code: 'UNKNOWN', messageKey: 'x' },
      { code: 'DATABASE' },
      { code: 'DATABASE', messageKey: 'x', details: 'stack trace' },
    ]) {
      expect(isCommandError(value)).toBe(false);
    }
  });

  it('normalizeError never passes raw text on', () => {
    expect(normalizeError(new Error('/home/user/secret.db'))).toBe(IPC_REJECTED);
  });
});
