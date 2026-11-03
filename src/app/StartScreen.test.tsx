import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { render, screen, within } from '@testing-library/react';
import { fireEvent } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLocale } from '@/i18n';
import { useLibrary } from '@/modules/library/libraryStore';
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

const closedLibrary = { state: 'closed', configuredPath: null, error: null, settingsNotice: null };

/**
 * Answers the library commands with "no library yet" and passes
 * system_check to `onCheck`.
 */
function mockApp(onCheck: (args: unknown) => unknown) {
  mockIPC((command, args) => {
    switch (command) {
      case 'library_status':
        return closedLibrary;
      case 'library_default_location':
        return null;
      case 'system_check':
        return onCheck(args);
      default:
        throw new Error(`unexpected command ${command}`);
    }
  });
}

beforeEach(() => {
  useLocale.setState({ locale: 'de' });
  useSystemStatus.setState({ status: { phase: 'running' } });
  useLibrary.setState(useLibrary.getInitialState(), true);
});

afterEach(() => {
  clearMocks();
});

describe('StartScreen', () => {
  it('requests every check and shows the results with icon and text', async () => {
    const requests: unknown[] = [];
    mockApp((args) => {
      requests.push(args);
      return response(allPassed);
    });

    render(<StartScreen />);

    expect(await screen.findByText('Alles bereit.')).toBeInTheDocument();
    expect(requests).toEqual([
      { request: { checks: ['database', 'migrations', 'foreignKeys', 'fullTextSearch'] } },
    ]);
    const list = within(screen.getByTestId('system-check')).getByRole('list');
    expect(within(list).getAllByRole('listitem')).toHaveLength(4);
    // Colour never carries the meaning alone: every row has a status word.
    expect(within(list).getAllByText('bereit')).toHaveLength(4);
    expect(screen.getByRole('heading', { level: 1, name: 'Project Write' })).toBeInTheDocument();
    expect(screen.getByTestId('start-footer')).toHaveTextContent('Version 0.1.0');
    expect(screen.getByTestId('start-footer')).toHaveTextContent('Linux');
  });

  it('marks failed checks in words', async () => {
    mockApp(() => response({ ...allPassed, fullTextSearch: false }));
    render(<StartScreen />);

    expect(await screen.findByText('Mindestens eine Prüfung ist fehlgeschlagen.')).toBeInTheDocument();
    const row = screen.getByText('Volltextsuche').closest('li');
    expect(row).toHaveAttribute('data-passed', 'false');
    expect(row).toHaveTextContent('fehlgeschlagen');
  });

  it('shows the translated error message of a command error', async () => {
    mockApp(() => {
      throw { code: 'DATABASE', messageKey: 'error.database' };
    });
    render(<StartScreen />);
    expect(await screen.findByText('Der Datenspeicher hat nicht geantwortet.')).toBeInTheDocument();
  });

  it('never shows raw rejection text', async () => {
    mockApp(() => {
      throw 'internal failure at /home/user/library/database/projectwrite.db';
    });
    render(<StartScreen />);
    expect(await screen.findByText('Die Aktion ist nicht erlaubt.')).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent('/home/user');
  });

  it('runs the checks again from the keyboard-accessible button', async () => {
    let calls = 0;
    mockApp(() => {
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
    mockApp(() => response(allPassed));
    render(<StartScreen />);
    expect(await screen.findByText('Everything is ready.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check again' })).toBeInTheDocument();
  });
});
