import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useLocale } from '@/i18n';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { LocationChoice } from '@/types/generated/LocationChoice';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import { LibraryPanel } from './LibraryPanel';
import { useLibrary } from './libraryStore';

const TOKEN = '0123456789abcdef0123456789abcdef';

const closed: LibraryStatus = { state: 'closed', configuredPath: null, error: null, settingsNotice: null };

function choice(overrides: Partial<LocationChoice> = {}): LocationChoice {
  return {
    token: TOKEN,
    purpose: 'create',
    displayPath: '/home/a/Dokumente/ProjectWrite',
    contents: 'missing',
    location: { kind: 'local' },
    ...overrides,
  };
}

function open(overrides: Partial<Extract<LibraryStatus, { state: 'open' }>['library']> = {}): LibraryStatus {
  return {
    state: 'open',
    settingsNotice: null,
    library: {
      displayPath: '/home/a/Dokumente/ProjectWrite',
      name: 'ProjectWrite',
      location: { kind: 'local' },
      locationAccepted: true,
      journalMode: 'wal',
      schemaVersion: 2,
      documentCount: 0,
      notes: { migratedFrom: null, migrationBackup: null, orphanedFiles: 0, missingFiles: 0 },
      ...overrides,
    },
  };
}

type Handler = (args: Record<string, unknown>) => unknown;

/** Routes commands to handlers and records every call. */
function mockCommands(handlers: Record<string, Handler>) {
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  mockIPC((command, args) => {
    const payload = args as Record<string, unknown>;
    calls.push({ command, args: payload });
    const handler = handlers[command];
    if (!handler) {
      throw new Error(`unexpected command ${command}`);
    }
    return handler(payload);
  });
  return calls;
}

function requestOf(calls: ReturnType<typeof mockCommands>, command: string) {
  return calls.filter((call) => call.command === command).map((call) => call.args['request']);
}

beforeEach(() => {
  useLocale.setState({ locale: 'de' });
  useLibrary.setState(useLibrary.getInitialState(), true);
});

afterEach(() => {
  clearMocks();
});

describe('LibraryPanel', () => {
  it('creates the first library at the default location', async () => {
    const calls = mockCommands({
      library_status: () => closed,
      library_default_location: () => choice(),
      library_create: () => open(),
    });
    render(<LibraryPanel />);

    expect(await screen.findByText(/Eine Bibliothek ist ein Ordner/)).toBeInTheDocument();
    expect(screen.getByTestId('library-default-location')).toHaveTextContent(
      'Ort: /home/a/Dokumente/ProjectWrite',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Bibliothek anlegen' }));

    expect(await screen.findByTestId('library-name')).toHaveTextContent('ProjectWrite');
    expect(requestOf(calls, 'library_create')).toEqual([{ location: TOKEN, acceptWarning: false }]);
    expect(screen.getByTestId('library-storage')).toHaveTextContent('Lokaler Datenträger');
    expect(screen.getByTestId('library-journal')).toHaveTextContent('WAL (Standard)');
    expect(screen.getByTestId('library-documents')).toHaveTextContent('0');
    expect(screen.getByRole('button', { name: 'Bibliothek umziehen …' })).toBeInTheDocument();
  });

  it('asks before creating a library in a sync folder', async () => {
    const synced = choice({ location: { kind: 'syncFolder', provider: 'OneDrive' } });
    const calls = mockCommands({
      library_status: () => closed,
      library_default_location: () => synced,
      library_create: () =>
        open({
          location: { kind: 'syncFolder', provider: 'OneDrive' },
          journalMode: 'rollback',
        }),
    });
    render(<LibraryPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Bibliothek anlegen' }));

    const dialog = await screen.findByRole('alertdialog', { name: 'Sync-Ordner erkannt' });
    expect(dialog).toHaveTextContent('OneDrive');
    expect(dialog).toHaveTextContent('Rollback-Modus');
    expect(requestOf(calls, 'library_create')).toEqual([]);
    const confirm = within(dialog).getByRole('button', { name: 'Trotzdem hier anlegen' });
    expect(confirm).toHaveFocus();
    fireEvent.click(confirm);

    expect(await screen.findByTestId('library-journal')).toHaveTextContent('Rollback-Journal');
    expect(requestOf(calls, 'library_create')).toEqual([{ location: TOKEN, acceptWarning: true }]);
    expect(screen.getByTestId('library-storage')).toHaveTextContent('Sync-Ordner (OneDrive)');
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  it('offers another location instead and cancels with Escape', async () => {
    const synced = choice({ location: { kind: 'networkDrive' } });
    const local = choice({ token: 'ffffffffffffffffffffffffffffffff', displayPath: '/home/a/Lokal' });
    const calls = mockCommands({
      library_status: () => closed,
      library_default_location: () => synced,
      library_choose_location: () => local,
      library_create: () => open({ displayPath: '/home/a/Lokal', name: 'Lokal' }),
    });
    render(<LibraryPanel />);

    fireEvent.click(await screen.findByRole('button', { name: 'Bibliothek anlegen' }));
    await screen.findByRole('alertdialog', { name: 'Netzlaufwerk erkannt' });
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Bibliothek anlegen' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Anderen Ort wählen' }));
    expect(await screen.findByTestId('library-name')).toHaveTextContent('Lokal');
    expect(requestOf(calls, 'library_choose_location')).toEqual([
      { purpose: 'create', title: 'Ort für die neue Bibliothek wählen' },
    ]);
    expect(requestOf(calls, 'library_create')).toEqual([
      { location: 'ffffffffffffffffffffffffffffffff', acceptWarning: false },
    ]);
  });

  it('opens a library that was moved only after a confirmation', async () => {
    const old = choice({ purpose: 'open', contents: 'library', displayPath: '/home/a/Alt' });
    const calls = mockCommands({
      library_status: () => closed,
      library_default_location: () => null,
      library_choose_location: () => old,
      library_open: (args) => {
        const request = args['request'] as { acceptMoved: boolean };
        if (!request.acceptMoved) {
          throw {
            code: 'LIBRARY_MOVED',
            messageKey: 'error.libraryMoved',
            details: { path: '/mnt/neu/ProjectWrite' },
          };
        }
        return open({ displayPath: '/home/a/Alt', name: 'Alt' });
      },
    });
    render(<LibraryPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Vorhandene Bibliothek öffnen …' }));

    const dialog = await screen.findByRole('alertdialog', { name: 'Bibliothek wurde umgezogen' });
    expect(dialog).toHaveTextContent('/mnt/neu/ProjectWrite');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Alte Kopie trotzdem öffnen' }));
    expect(await screen.findByTestId('library-name')).toHaveTextContent('Alt');
    expect(requestOf(calls, 'library_open')).toEqual([
      { location: TOKEN, acceptMoved: false },
      { location: TOKEN, acceptMoved: true },
    ]);
  });

  it('offers to open a library found where a new one should be created', async () => {
    const existing = choice({ contents: 'library' });
    const calls = mockCommands({
      library_status: () => closed,
      library_default_location: () => existing,
      library_open: () => open(),
    });
    render(<LibraryPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Bibliothek anlegen' }));
    const dialog = await screen.findByRole('alertdialog', {
      name: 'In diesem Ordner liegt bereits eine Bibliothek.',
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Diese Bibliothek öffnen' }));
    expect(await screen.findByTestId('library-name')).toBeInTheDocument();
    expect(requestOf(calls, 'library_create')).toEqual([]);
    expect(requestOf(calls, 'library_open')).toEqual([{ location: TOKEN, acceptMoved: false }]);
  });

  it('explains why the remembered library is not open and tries again', async () => {
    let attempts = 0;
    const locked = { code: 'LIBRARY_LOCKED', messageKey: 'error.libraryLocked' };
    const calls = mockCommands({
      library_status: () => ({ ...closed, configuredPath: '/home/a/ProjectWrite', error: locked }),
      library_default_location: () => choice(),
      library_reopen: () => {
        attempts += 1;
        if (attempts === 1) {
          throw locked;
        }
        return open();
      },
    });
    render(<LibraryPanel />);

    expect(await screen.findByTestId('library-open-error')).toHaveTextContent(
      'bereits in einer anderen Instanz von Project Write geöffnet',
    );
    expect(screen.getByText('Zuletzt verwendet: /home/a/ProjectWrite')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Erneut versuchen' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('anderen Instanz');
    fireEvent.click(screen.getByRole('button', { name: 'Erneut versuchen' }));
    expect(await screen.findByTestId('library-name')).toBeInTheDocument();
    expect(requestOf(calls, 'library_reopen')).toHaveLength(2);
  });

  it('moves the library with progress and keeps the old folder', async () => {
    let finish: ((status: LibraryStatus) => void) | undefined;
    let channelId: number | undefined;
    const target = choice({ purpose: 'move', displayPath: '/home/a/Neu', contents: 'missing' });
    const calls = mockCommands({
      library_status: () => open(),
      library_default_location: () => null,
      library_choose_location: () => target,
      library_move: (args) => {
        channelId = (args['onProgress'] as { id: number }).id;
        return new Promise((resolve) => {
          finish = resolve;
        });
      },
    });
    render(<LibraryPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Bibliothek umziehen …' }));

    expect(await screen.findByTestId('library-moving')).toHaveTextContent('Bibliothek wird umgezogen');
    expect(requestOf(calls, 'library_move')).toEqual([{ location: TOKEN, acceptWarning: false }]);
    const progress: MoveProgress = {
      phase: 'copying',
      doneBytes: 1536,
      totalBytes: 3 * 1024 * 1024,
      doneFiles: 1,
      totalFiles: 3,
    };
    act(() => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { runCallback: (id: number, data: unknown) => void };
        }
      ).__TAURI_INTERNALS__;
      internals.runCallback(channelId ?? -1, { message: progress, index: 0 });
    });
    expect(screen.getByTestId('library-progress')).toHaveTextContent(
      'Kopieren: 1 von 3 Dateien (1,5 KB von 3 MB)',
    );

    act(() => {
      finish?.(open({ displayPath: '/home/a/Neu', name: 'Neu' }));
    });
    expect(await screen.findByTestId('library-path')).toHaveTextContent('/home/a/Neu');
    expect(screen.getByTestId('library-move-notice')).toHaveTextContent(
      'Der alte Ordner bleibt erhalten, bis Sie ihn entfernen: /home/a/Dokumente/ProjectWrite',
    );
  });

  it('refuses a move target with other files before calling Rust', async () => {
    const calls = mockCommands({
      library_status: () => open(),
      library_default_location: () => null,
      library_choose_location: () => choice({ purpose: 'move', contents: 'library' }),
    });
    render(<LibraryPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Bibliothek umziehen …' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'In diesem Ordner liegt bereits eine Bibliothek.',
    );
    expect(requestOf(calls, 'library_move')).toEqual([]);
  });

  it('shows the warning for an unconfirmed sync folder until it is dismissed', async () => {
    const synced = { location: { kind: 'syncFolder', provider: 'Dropbox' } as const };
    const calls = mockCommands({
      library_status: () => open({ ...synced, journalMode: 'rollback', locationAccepted: false }),
      library_default_location: () => null,
      library_accept_location: () => open({ ...synced, journalMode: 'rollback', locationAccepted: true }),
    });
    render(<LibraryPanel />);
    const warning = await screen.findByTestId('library-location-warning');
    expect(warning).toHaveTextContent('Dropbox');
    fireEvent.click(within(warning).getByRole('button', { name: 'Hinweis ausblenden' }));
    await screen.findByTestId('library-name');
    expect(await screen.findByTestId('library-journal')).toHaveTextContent('Rollback-Journal');
    expect(screen.queryByTestId('library-location-warning')).not.toBeInTheDocument();
    expect(requestOf(calls, 'library_accept_location')).toEqual([{}]);
  });

  it('reports findings of the last open', async () => {
    mockCommands({
      library_status: () =>
        open({
          notes: {
            migratedFrom: 1,
            migrationBackup: 'backups/pre-migration/projectwrite-schema-1-x.db',
            orphanedFiles: 2,
            missingFiles: 1,
          },
        }),
      library_default_location: () => null,
    });
    render(<LibraryPanel />);
    expect(await screen.findByText(/von Schema 1 aktualisiert/)).toHaveTextContent(
      'backups/pre-migration/projectwrite-schema-1-x.db',
    );
    expect(screen.getByText(/2 Dateien ohne Datenbankeintrag/)).toBeInTheDocument();
    expect(screen.getByText('Bei 1 Dokumenten fehlt die Datei.')).toBeInTheDocument();
  });

  it('never shows raw rejection text', async () => {
    mockCommands({
      library_status: () => {
        throw 'panic at /home/a/secret/projectwrite.db';
      },
      library_default_location: () => null,
    });
    render(<LibraryPanel />);
    expect(await screen.findByTestId('library-open-error')).toHaveTextContent(
      'Die Aktion ist nicht erlaubt.',
    );
    expect(document.body).not.toHaveTextContent('/home/a/secret');
  });

  it('is available in English', async () => {
    useLocale.setState({ locale: 'en' });
    mockCommands({
      library_status: () => closed,
      library_default_location: () => choice(),
    });
    render(<LibraryPanel />);
    expect(await screen.findByRole('button', { name: 'Create library' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open existing library …' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
  });
});
