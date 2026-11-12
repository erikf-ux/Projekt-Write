/**
 * The app: first start (language and library) until a library is open,
 * then the workbench. Commands, shortcuts, native menu and overlays are
 * set up here once.
 */
import { useCallback, useEffect, useRef } from 'react';
import { create } from 'zustand';
import { NoticeBar } from '@/components/NoticeBar';
import { useLocale, useT } from '@/i18n';
import { CommandPalette, type PaletteEntry } from '@/modules/commands/CommandPalette';
import { formatChord } from '@/modules/commands/keymap';
import { availableCommands, findCommand, registerCommands, runCommand } from '@/modules/commands/registry';
import { installShortcuts } from '@/modules/commands/shortcuts';
import type { CommandCategory } from '@/modules/commands/types';
import { DropZone } from '@/modules/documents/DropZone';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { notePlainPasteChord } from '@/modules/editor/paste';
import { useLibrary } from '@/modules/library/libraryStore';
import { SettingsDialog } from '@/modules/settings/SettingsDialog';
import { useUi } from '@/modules/settings/uiStore';
import { call, subscribeUiEvents } from '@/services/ipc';
import type { MenuCommand } from '@/types/generated/MenuCommand';
import type { MenuItemState } from '@/types/generated/MenuItemState';
import { currentBindings } from './bindings';
import { ClosePrompt } from './ClosePrompt';
import { PromptDialog } from '@/components/prompt/PromptDialog';
import { createFormatCommands } from './formatCommands';
import { createAppCommands } from './commands';
import { requestQuit } from './lifecycle';
import { StartScreen } from './StartScreen';
import { Workbench } from './Workbench';

registerCommands([...createAppCommands(), ...createFormatCommands()]);

/** Whether this session started without an open library (first start). */
const useFirstStart = create<{ seen: boolean }>(() => ({ seen: false }));
useLibrary.subscribe((state) => {
  if (state.status?.state === 'closed' && !useFirstStart.getState().seen) {
    useFirstStart.setState({ seen: true });
  }
});

/** Every item of the native menu (the compiler checks completeness). */
const MENU_COMMANDS: Record<MenuCommand, true> = {
  'document.new': true,
  'document.save': true,
  'document.openFile': true,
  'document.saveAs': true,
  'document.print': true,
  'export.pdf': true,
  'backup.create': true,
  'tab.close': true,
  'settings.open': true,
  'app.quit': true,
  'palette.show': true,
  'sidebar.toggle': true,
  'focus.toggle': true,
  'zoom.in': true,
  'zoom.out': true,
  'zoom.reset': true,
  'edit.undo': true,
  'edit.redo': true,
  'find.show': true,
  'replace.show': true,
  'search.focus': true,
  'editor.toggleSource': true,
  'statistics.show': true,
  'format.bold': true,
  'format.italic': true,
  'format.underline': true,
  'format.strike': true,
  'format.code': true,
  'format.paragraph': true,
  'format.heading1': true,
  'format.heading2': true,
  'format.heading3': true,
  'format.bulletList': true,
  'format.orderedList': true,
  'format.taskList': true,
  'format.blockquote': true,
  'format.codeBlock': true,
  'format.link': true,
  'format.image': true,
  'format.pdfReference': true,
  'format.horizontalRule': true,
  'format.table': true,
};

const categoryKeys = {
  document: 'category.document',
  tab: 'category.tab',
  edit: 'category.edit',
  view: 'category.view',
  app: 'category.app',
} as const satisfies Record<CommandCategory, string>;

/** Text editing the webview does itself (reserved chords, ADR 0012). */
const FIXED_SHORTCUTS: Partial<Record<MenuCommand, string>> = {
  'edit.undo': 'Ctrl+Z',
  'edit.redo': 'Ctrl+Y',
};

function shortcutText(id: string): string | undefined {
  const chord = currentBindings().get(id as MenuCommand)?.[0] ?? FIXED_SHORTCUTS[id as MenuCommand];
  const platform = useUi.getState().state?.platform ?? 'windows';
  return chord ? formatChord(chord, useLocale.getState().locale, platform) : undefined;
}

/** Enabled states and shortcut texts of the native menu items. */
function menuItems(): Partial<Record<MenuCommand, MenuItemState>> {
  const items: Partial<Record<MenuCommand, MenuItemState>> = {};
  for (const id of Object.keys(MENU_COMMANDS) as MenuCommand[]) {
    items[id] = { enabled: findCommand(id)?.isAvailable() ?? false, shortcut: shortcutText(id) ?? null };
  }
  return items;
}

export function App() {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const uiState = useUi((state) => state.state);
  const loadError = useUi((state) => state.loadError);
  const paletteOpen = useUi((state) => state.paletteOpen);
  const settingsSection = useUi((state) => state.settingsSection);
  const closePrompt = useUi((state) => state.closePrompt);
  const notice = useUi((state) => state.notice);
  const lastError = useWorkspace((state) => state.lastError);
  const status = useLibrary((state) => state.status);
  const sawFirstStart = useFirstStart((state) => state.seen);
  const lastMenu = useRef('');

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    void useUi.getState().load();
    void useLibrary.getState().load();
  }, []);

  // Menu clicks and close requests from Rust.
  useEffect(() => {
    void subscribeUiEvents((event) => {
      if (event.kind === 'command') {
        void runCommand(event.command);
      } else if (event.kind === 'openRequested') {
        void useWorkspace.getState().takeOpenRequests();
      } else {
        void requestQuit();
      }
    });
  }, []);

  // Shortcuts, everywhere in the window.
  useEffect(
    () =>
      installShortcuts({
        platform: () => useUi.getState().state?.platform ?? 'windows',
        bindings: currentBindings,
        isRecording: () => useUi.getState().recording,
        onConflict: (chord) => {
          const platform = useUi.getState().state?.platform ?? 'windows';
          useUi.getState().showNotice({
            tone: 'error',
            key: 'notice.shortcutConflict',
            params: { chord: formatChord(chord, useLocale.getState().locale, platform) },
          });
        },
      }),
    [],
  );

  // Ctrl+Shift+V pastes plain text, also where the webview does not.
  useEffect(() => {
    window.addEventListener('keydown', notePlainPasteChord, { capture: true });
    return () => {
      window.removeEventListener('keydown', notePlainPasteChord, { capture: true });
    };
  }, []);

  // Escape ends focus mode (after dialogs and menus had their turn).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const ui = useUi.getState();
      // ProseMirror cancels every Escape in the writing surface without using it.
      const fromEditor = event.target instanceof Element && event.target.closest('.ProseMirror') !== null;
      if (
        event.key === 'Escape' &&
        (!event.defaultPrevented || fromEditor) &&
        ui.focusMode &&
        !ui.paletteOpen &&
        ui.settingsSection === null
      ) {
        event.preventDefault();
        void ui.setFocusMode(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  // Keep the native menu in step: only available commands are enabled.
  useEffect(() => {
    let scheduled = false;
    const sync = () => {
      if (scheduled) {
        return;
      }
      scheduled = true;
      queueMicrotask(() => {
        scheduled = false;
        const items = menuItems();
        const json = JSON.stringify(items);
        if (json !== lastMenu.current) {
          lastMenu.current = json;
          void call('ui_menu_state', { items });
        }
      });
    };
    sync();
    const unsubscribe = [
      useUi.subscribe(sync),
      useWorkspace.subscribe(sync),
      useLibrary.subscribe(sync),
      useLocale.subscribe(sync),
    ];
    return () => {
      for (const stop of unsubscribe) {
        stop();
      }
    };
  }, []);

  // The first start ends when a library is open.
  const libraryOpen = status?.state === 'open' || status?.state === 'moving';

  const closePalette = useCallback(() => {
    useUi.getState().setPaletteOpen(false);
  }, []);
  const closeNotice = useCallback(() => {
    useUi.getState().showNotice(null);
    useWorkspace.setState({ lastError: null });
  }, []);

  const paletteEntries: PaletteEntry[] = paletteOpen
    ? availableCommands()
        .filter((command) => command.id !== 'palette.show')
        .map((command) => ({
          id: command.id,
          title: t(command.titleKey),
          category: t(categoryKeys[command.category]),
          shortcut: shortcutText(command.id),
        }))
    : [];

  if (uiState === null) {
    return (
      <main className="flex h-full items-center justify-center bg-surface text-muted">
        <p role="status" className="text-ui">
          {loadError ? t('error.internal') : t('app.loading')}
        </p>
      </main>
    );
  }

  const shownNotice =
    notice ??
    (lastError ? { tone: 'error' as const, key: 'notice.actionFailed' as const, error: lastError } : null);

  return (
    <>
      {libraryOpen ? (
        <DropZone>
          <Workbench openEmptyDocument={sawFirstStart} />
        </DropZone>
      ) : (
        <StartScreen />
      )}
      {paletteOpen ? (
        <CommandPalette entries={paletteEntries} onRun={(id) => void runCommand(id)} onClose={closePalette} />
      ) : null}
      {settingsSection !== null ? <SettingsDialog /> : null}
      {closePrompt ? <ClosePrompt /> : null}
      <PromptDialog />
      <NoticeBar notice={shownNotice} onClose={closeNotice} />
    </>
  );
}
