import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { render, screen, within } from '@testing-library/react';
import { fireEvent } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLocale } from '@/i18n';
import { StartScreen } from './StartScreen';
import { useSystemStatus } from './systemStatusStore';

function response(passed: Record<string, boolean>) {
  return {
    appVersion: '0.1.0',
    platform: 'linux',
    sqliteVersion: '3.50.0',
    schemaVersion: 1,
    results: Object.entries(passed).map(([check, ok]) => ({ check, passed: ok })),
  };
}

const allPassed = { database: true, migrations: true, foreignKeys: true, fullTextSearch: true };

beforeEach(() => {
  useLocale.setState({ locale: 'de' });
  useSystemStatus.setState({ status: { phase: 'running' } });
});

afterEach(() => {
  clearMocks();
});

describe('StartScreen', () => {
  it('requests every check and shows the results with icon and text', async () => {
    const requests: unknown[] = [];
    mockIPC((command, args) => {
      requests.push({ command, args });
      return response(allPassed);
    });

    render(<StartScreen />);

    expect(await screen.findByText('Alles bereit.')).toBeInTheDocument();
    expect(requests[0]).toEqual({
      command: 'system_check',
      args: { request: { checks: ['database', 'migrations', 'foreignKeys', 'fullTextSearch'] } },
    });
    const list = screen.getByRole('list');
    expect(within(list).getAllByRole('listitem')).toHaveLength(4);
    // Colour never carries the meaning alone: every row has a status word.
    expect(within(list).getAllByText('bereit')).toHaveLength(4);
    expect(screen.getByRole('heading', { level: 1, name: 'Project Write' })).toBeInTheDocument();
    expect(screen.getByTestId('start-footer')).toHaveTextContent('Version 0.1.0');
    expect(screen.getByTestId('start-footer')).toHaveTextContent('Linux');
  });

  it('marks failed checks in words', async () => {
    mockIPC(() => response({ ...allPassed, fullTextSearch: false }));
    render(<StartScreen />);

    expect(await screen.findByText('Mindestens eine Prüfung ist fehlgeschlagen.')).toBeInTheDocument();
    const row = screen.getByText('Volltextsuche').closest('li');
    expect(row).toHaveAttribute('data-passed', 'false');
    expect(row).toHaveTextContent('fehlgeschlagen');
  });

  it('shows the translated error message of a command error', async () => {
    mockIPC(() => {
      throw { code: 'DATABASE', messageKey: 'error.database' };
    });
    render(<StartScreen />);
    expect(await screen.findByText('Der Datenspeicher hat nicht geantwortet.')).toBeInTheDocument();
  });

  it('never shows raw rejection text', async () => {
    mockIPC(() => {
      throw 'internal failure at /home/user/library/database/projectwrite.db';
    });
    render(<StartScreen />);
    expect(await screen.findByText('Die Aktion ist nicht erlaubt.')).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent('/home/user');
  });

  it('runs the checks again from the keyboard-accessible button', async () => {
    let calls = 0;
    mockIPC(() => {
      calls += 1;
      return response(allPassed);
    });
    render(<StartScreen />);
    await screen.findByText('Alles bereit.');

    const button = screen.getByRole('button', { name: 'Erneut prüfen' });
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.click(button);
    await screen.findByText('Alles bereit.');
    expect(calls).toBe(2);
  });

  it('is available in English', async () => {
    useLocale.setState({ locale: 'en' });
    mockIPC(() => response(allPassed));
    render(<StartScreen />);
    expect(await screen.findByText('Everything is ready.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check again' })).toBeInTheDocument();
  });
});
