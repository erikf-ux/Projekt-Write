import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { PromptDialog } from '@/components/prompt/PromptDialog';
import { translate, useLocale } from '@/i18n';
import { useBackup } from '@/modules/backup/backupStore';
import { useLibrary } from '@/modules/library/libraryStore';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { SettingsApplied } from '@/types/generated/SettingsApplied';
import type { SettingsImportPreview } from '@/types/generated/SettingsImportPreview';
import type { UiState } from '@/types/generated/UiState';
import { AdvancedSettings } from './AdvancedSettings';
import { formatSettingValue, settingLabel } from './settingsTransfer';
import { useUi } from './uiStore';

const TOKEN = '0123456789abcdef0123456789abcdef';
const t = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate('de', key, params);

const uiState: UiState = {
  settings: {
    language: 'system',
    theme: 'system',
    zoom: 1.5,
    sidebar: { visible: true, width: 260 },
    limitLineWidth: true,
    autosaveExternal: false,
    readingTime: { visible: false, wordsPerMinute: 200 },
    keybindings: {},
    languageCheck: {
      enabled: true,
      rules: true,
      autocorrect: false,
      autocorrectAsked: true,
      defaultLanguage: null,
      detect: true,
    },
  },
  locale: 'de',
  platform: 'linux',
  readOnly: false,
  customThemes: [],
};

const openLibrary: LibraryStatus = {
  state: 'open',
  settingsNotice: null,
  library: {
    displayPath: '/home/a/ProjectWrite',
    name: 'ProjectWrite',
    location: { kind: 'local' },
    locationAccepted: true,
    journalMode: 'wal',
    schemaVersion: 2,
    documentCount: 0,
    notes: { migratedFrom: null, migrationBackup: null, orphanedFiles: 0, missingFiles: 0 },
  },
};

const preview: SettingsImportPreview = {
  token: TOKEN,
  exportedAt: '2026-11-17T12:00:00.000Z',
  changes: [
    { area: 'app', key: 'zoom', before: 1.5, after: 1.25 },
    { area: 'app', key: 'theme', before: 'system', after: 'dark' },
    { area: 'library', key: 'trash.retentionDays', before: 40, after: 12 },
  ],
  skipped: [{ key: 'app.fontUrl', reason: 'unknown' }],
};

function applied(change: Partial<UiState['settings']>, libraryChanged: boolean): SettingsApplied {
  return { ui: { ...uiState, settings: { ...uiState.settings, ...change } }, libraryChanged };
}

type Handler = (request: unknown) => unknown;

/** Routes commands to handlers and records every request. */
function mockCommands(handlers: Record<string, Handler>) {
  const calls: { command: string; request: unknown }[] = [];
  mockIPC((command, args) => {
    const request = (args as Record<string, unknown>)['request'];
    calls.push({ command, request });
    const handler = handlers[command];
    if (!handler) {
      throw new Error(`unexpected command ${command}`);
    }
    return handler(request);
  });
  return calls;
}

const locations = () => ({
  config: '/home/a/.config/pw',
  logs: '/home/a/.local/pw/logs',
  library: '/home/a/ProjectWrite',
});

beforeEach(() => {
  useLocale.setState({ locale: 'de' });
  useUi.setState({ state: uiState });
  useLibrary.setState({ status: openLibrary });
  useBackup.setState({ overview: null, error: null });
});

afterEach(() => {
  clearMocks();
});

describe('formatSettingValue', () => {
  it('shows values the way the settings show them', () => {
    expect(formatSettingValue(t, 'de', 'zoom', 1.25)).toBe('125 %');
    expect(formatSettingValue(t, 'de', 'limitLineWidth', false)).toBe('aus');
    expect(formatSettingValue(t, 'de', 'backup.automatic', true)).toBe('an');
    expect(formatSettingValue(t, 'de', 'mesh.suggestionThreshold', 0.35)).toBe('0,35');
    expect(formatSettingValue(t, 'en', 'mesh.suggestionThreshold', 0.35)).toBe('0.35');
    expect(formatSettingValue(t, 'de', 'theme', 'dark')).toBe('Dunkel');
    expect(formatSettingValue(t, 'de', 'language', 'system')).toBe('Wie das System');
    expect(formatSettingValue(t, 'de', 'language', 'en')).toBe('English');
    const id = '0123456789abcdef0123456789abcdef';
    const theme = { id, name: 'Nacht', base: 'dark' as const, colors: {} as never };
    expect(formatSettingValue(t, 'de', 'theme', `custom:${id}`, [theme])).toBe('Nacht');
    expect(formatSettingValue(t, 'de', 'theme', `custom:${id}`)).toBe(`custom:${id}`);
    expect(formatSettingValue(t, 'de', 'sidebar', { visible: true, width: 300 })).toBe(
      'visible: an; width: 300',
    );
    expect(
      formatSettingValue(t, 'de', 'keybindings', { 'palette.show': ['Ctrl+K', 'F1'], 'focus.toggle': [] }),
    ).toBe('palette.show: Ctrl+K, F1; focus.toggle: –');
    expect(formatSettingValue(t, 'de', 'keybindings', {})).toBe('–');
    expect(formatSettingValue(t, 'de', 'languageCheck', { defaultLanguage: null })).toBe(
      'defaultLanguage: –',
    );
  });
});

describe('settingLabel', () => {
  it('names known keys and shows unknown ones as they are', () => {
    expect(settingLabel(t, 'app', 'zoom')).toBe('Zoom');
    expect(settingLabel(t, 'library', 'trash.retentionDays')).toBe(t('settings.library.trashRetention'));
    expect(settingLabel(t, 'library', 'future.setting')).toBe('future.setting');
  });
});

describe('AdvancedSettings', () => {
  it('previews an import and applies it by token only', async () => {
    const calls = mockCommands({
      system_locations: locations,
      settings_import_preview: () => preview,
      settings_import_apply: () => applied({ zoom: 1.25, theme: 'dark' }, true),
      backup_overview: () => null,
    });
    render(<AdvancedSettings />);
    fireEvent.click(screen.getByTestId('settings-import'));
    const dialog = await screen.findByTestId('settings-import-preview');
    const app = within(dialog).getByTestId('settings-changes-app');
    expect(within(app).getByText('Zoom').closest('tr')).toHaveTextContent('Zoom150 %125 %');
    expect(within(dialog).getByTestId('settings-changes-library')).toHaveTextContent(
      t('settings.library.trashRetention'),
    );
    expect(within(dialog).getByTestId('settings-import-skipped')).toHaveTextContent('app.fontUrl');
    expect(useUi.getState().state?.settings.zoom).toBe(1.5);

    fireEvent.click(within(dialog).getByTestId('settings-import-apply'));
    await screen.findByText(t('settings.transfer.imported'));
    expect(screen.queryByTestId('settings-import-preview')).not.toBeInTheDocument();
    expect(calls.find((call) => call.command === 'settings_import_apply')?.request).toEqual({ token: TOKEN });
    expect(useUi.getState().state?.settings.theme).toBe('dark');
    expect(document.documentElement.dataset['theme']).toBe('dark');
    // Views of library settings load them again.
    await waitFor(() => {
      expect(calls.some((call) => call.command === 'backup_overview')).toBe(true);
    });
  });

  it('applies nothing when the preview is cancelled', async () => {
    const calls = mockCommands({
      system_locations: locations,
      settings_import_preview: () => preview,
    });
    render(<AdvancedSettings />);
    fireEvent.click(screen.getByTestId('settings-import'));
    await screen.findByTestId('settings-import-preview');
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(screen.queryByTestId('settings-import-preview')).not.toBeInTheDocument();
    expect(calls.some((call) => call.command === 'settings_import_apply')).toBe(false);
    expect(useUi.getState().state?.settings.zoom).toBe(1.5);
  });

  it('shows why a file is refused', async () => {
    mockCommands({
      system_locations: locations,
      settings_import_preview: () => {
        throw {
          code: 'SETTINGS_VALUE_INVALID',
          messageKey: 'error.settingsValueInvalid',
          details: { key: 'app.zoom', reason: 'zoom range' },
        };
      },
    });
    render(<AdvancedSettings />);
    fireEvent.click(screen.getByTestId('settings-import'));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      t('error.settingsValueInvalid', { key: 'app.zoom' }),
    );
    expect(screen.queryByTestId('settings-import-preview')).not.toBeInTheDocument();
  });

  it('resets only after a confirmation', async () => {
    const calls = mockCommands({
      system_locations: locations,
      settings_reset: () => applied({ zoom: 1 }, false),
    });
    render(
      <>
        <AdvancedSettings />
        <PromptDialog />
      </>,
    );
    fireEvent.click(screen.getByTestId('settings-reset-app'));
    let prompt = await screen.findByTestId('settings-reset-confirm');
    fireEvent.click(within(prompt).getByText(t('settings.transfer.cancel')));
    expect(calls.some((call) => call.command === 'settings_reset')).toBe(false);

    fireEvent.click(screen.getByTestId('settings-reset-app'));
    prompt = await screen.findByTestId('settings-reset-confirm');
    fireEvent.click(within(prompt).getByText(t('settings.reset.confirm')));
    await screen.findByText(t('settings.reset.appDone'));
    expect(calls.find((call) => call.command === 'settings_reset')?.request).toEqual({ area: 'app' });
    expect(useUi.getState().state?.settings.zoom).toBe(1);
  });

  it('shows the storage locations and opens them by name', async () => {
    const calls = mockCommands({
      system_locations: locations,
      system_open_location: () => null,
    });
    render(<AdvancedSettings />);
    expect(await screen.findByText('/home/a/.config/pw')).toBeInTheDocument();
    expect(screen.getByTestId('advanced-extensions')).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: t('settings.locations.openLabel', { name: 'Protokoll' }) }),
    );
    await waitFor(() => {
      expect(calls.find((call) => call.command === 'system_open_location')?.request).toEqual({
        location: 'logs',
      });
    });
  });

  it('cannot reset or open the library while none is open', async () => {
    useLibrary.setState({
      status: { state: 'closed', configuredPath: null, error: null, settingsNotice: null },
    });
    mockCommands({
      system_locations: () => ({ ...locations(), library: null }),
    });
    render(<AdvancedSettings />);
    expect(await screen.findByText(t('settings.locations.noLibrary'))).toBeInTheDocument();
    expect(screen.getByTestId('settings-reset-library')).toBeDisabled();
    expect(screen.getByTestId('location-library-open')).toBeDisabled();
  });
});
