import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { useUi } from '@/modules/settings/uiStore';
import type { MenuCommand } from '@/types/generated/MenuCommand';
import type { UiState } from '@/types/generated/UiState';
import { createAppCommands } from './commands';

const commands = createAppCommands();
const available = () =>
  commands
    .filter((command) => command.isAvailable())
    .map((command) => command.id)
    .sort();

const uiState: UiState = {
  settings: {
    language: 'de',
    theme: 'system',
    zoom: 1,
    sidebar: { visible: true, width: 260 },
    limitLineWidth: true,
    keybindings: {},
  },
  locale: 'de',
  platform: 'linux',
  readOnly: false,
  customThemes: [],
};

const openLibrary = {
  state: 'open',
  settingsNotice: null,
  library: {
    displayPath: '/x/Bibliothek',
    name: 'Bibliothek',
    location: { kind: 'local' },
    locationAccepted: true,
    journalMode: 'wal',
    schemaVersion: 3,
    documentCount: 0,
    notes: { removedTempFiles: 0, orphanedFiles: [], unknownEntries: [] },
  },
} as never;

function openDocument(id: string, state: 'saved' | 'pending' = 'saved') {
  useWorkspace.setState((current) => ({
    tabs: [...current.tabs, id],
    active: id,
    open: {
      ...current.open,
      [id]: {
        id,
        title: id,
        format: 'md',
        loadedText: '',
        savedText: '',
        revision: 0,
        state,
        error: null,
        losses: [],
        view: 'rich',
        viewIssues: [],
        loaded: true,
      },
    },
  }));
}

describe('command availability (the palette shows only working commands)', () => {
  beforeEach(() => {
    useUi.setState({ state: uiState, focusMode: false, paletteOpen: false, settingsSection: null });
    useWorkspace.setState({ tabs: [], active: null, open: {}, documents: [] });
    useLibrary.setState({
      status: { state: 'closed', configuredPath: null, error: null, settingsNotice: null },
    });
  });
  afterEach(() => {
    useUi.setState({ state: null });
  });

  it('offers no document functions without a library', () => {
    const ids = available();
    expect(ids).not.toContain('document.new');
    expect(ids).not.toContain('sidebar.toggle');
    expect(ids).not.toContain('focus.toggle');
    expect(ids).toContain('palette.show');
    expect(ids).toContain('app.quit');
  });

  it('offers tab functions only for open tabs and saving only for unsaved text', () => {
    useLibrary.setState({ status: openLibrary });
    expect(available()).toContain('document.new');
    for (const id of ['tab.close', 'tab.next', 'document.save', 'document.rename', 'editor.focus']) {
      expect(available()).not.toContain(id);
    }
    openDocument('a');
    expect(available()).toContain('tab.close');
    expect(available()).not.toContain('tab.next');
    expect(available()).not.toContain('document.save');
    openDocument('b', 'pending');
    expect(available()).toEqual(
      expect.arrayContaining(['tab.next', 'tab.previous', 'tab.moveLeft', 'document.save']),
    );
    expect(available()).not.toContain('tab.moveRight');
  });

  it('hides settings that are already in effect', () => {
    expect(available()).not.toContain('zoom.reset');
    expect(available()).not.toContain('theme.system');
    expect(available()).toContain('theme.dark');
    useUi.setState({ state: { ...uiState, settings: { ...uiState.settings, zoom: 2, theme: 'dark' } } });
    expect(available()).not.toContain('zoom.in');
    expect(available()).not.toContain('theme.dark');
    expect(available()).toEqual(expect.arrayContaining(['zoom.reset', 'zoom.out', 'theme.system']));
  });

  it('registers every item of the native menu', () => {
    const menu: Record<MenuCommand, true> = {
      'document.new': true,
      'document.save': true,
      'tab.close': true,
      'settings.open': true,
      'app.quit': true,
      'palette.show': true,
      'sidebar.toggle': true,
      'focus.toggle': true,
      'zoom.in': true,
      'zoom.out': true,
      'zoom.reset': true,
    };
    const ids = new Set(commands.map((command) => command.id));
    for (const id of Object.keys(menu)) {
      expect(ids.has(id as MenuCommand), id).toBe(true);
    }
  });

  it('has a title in both languages for every command', async () => {
    const { dictionaries } = await import('@/i18n');
    for (const command of commands) {
      expect(dictionaries.de[command.titleKey]).toBeTruthy();
      expect(dictionaries.en[command.titleKey]).toBeTruthy();
    }
  });
});
