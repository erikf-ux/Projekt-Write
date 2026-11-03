/**
 * The commands of Phase 3 (ADR 0012). `isAvailable` decides what the
 * command palette shows and which menu items are enabled: only what works
 * right now (section 25, Phase 3: "Die Command Palette zeigt nur
 * funktionierende Befehle").
 */
import { translate, useLocale } from '@/i18n';
import type { CommandDefinition } from '@/modules/commands/types';
import { freeTitle, useWorkspace } from '@/modules/documents/workspaceStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { nextZoom, useUi } from '@/modules/settings/uiStore';
import type { UiSettings } from '@/types/generated/UiSettings';
import { focusEditor, focusExplorer } from './focus';
import { requestQuit } from './lifecycle';

const ui = () => useUi.getState();
const workspace = () => useWorkspace.getState();
const settings = (): UiSettings | null => ui().state?.settings ?? null;
const libraryOpen = () => useLibrary.getState().status?.state === 'open';
const activeIndex = () => {
  const { tabs, active } = workspace();
  return active ? tabs.indexOf(active) : -1;
};

async function newDocument(): Promise<void> {
  const title = freeTitle(translate(useLocale.getState().locale, 'document.untitled'), workspace().documents);
  if (await workspace().createDocument(title)) {
    focusEditor();
  }
}

function switchTab(step: 1 | -1): void {
  const { tabs } = workspace();
  const next = tabs[(activeIndex() + step + tabs.length) % tabs.length];
  if (next) {
    workspace().activate(next);
    focusEditor();
  }
}

function setTheme(theme: string): CommandDefinition['run'] {
  return () =>
    ui()
      .update({ theme })
      .then(() => undefined);
}

export function createAppCommands(): CommandDefinition[] {
  return [
    {
      id: 'document.new',
      category: 'document',
      titleKey: 'command.document.new',
      isAvailable: libraryOpen,
      run: newDocument,
    },
    {
      id: 'document.save',
      category: 'document',
      titleKey: 'command.document.save',
      isAvailable: () => {
        const { active, open } = workspace();
        const state = active ? open[active]?.state : undefined;
        return state === 'pending' || state === 'error';
      },
      run: async () => {
        const { active } = workspace();
        if (active) {
          await workspace().save(active);
        }
      },
    },
    {
      id: 'document.rename',
      category: 'document',
      titleKey: 'command.document.rename',
      isAvailable: () => workspace().active !== null && !ui().focusMode,
      run: async () => {
        const current = settings();
        if (current && !current.sidebar.visible) {
          await ui().update({ sidebar: { ...current.sidebar, visible: true } });
        }
        workspace().setRenaming(workspace().active);
      },
    },
    {
      id: 'tab.close',
      category: 'tab',
      titleKey: 'command.tab.close',
      isAvailable: () => workspace().active !== null,
      run: async () => {
        const { active } = workspace();
        if (active) {
          await workspace().closeTab(active);
          focusEditor();
        }
      },
    },
    {
      id: 'tab.closeOthers',
      category: 'tab',
      titleKey: 'command.tab.closeOthers',
      isAvailable: () => workspace().tabs.length > 1,
      run: async () => {
        const { active } = workspace();
        if (active) {
          await workspace().closeOthers(active);
        }
      },
    },
    {
      id: 'tab.closeAll',
      category: 'tab',
      titleKey: 'command.tab.closeAll',
      isAvailable: () => workspace().tabs.length > 0,
      run: () => workspace().closeAll(),
    },
    {
      id: 'tab.next',
      category: 'tab',
      titleKey: 'command.tab.next',
      isAvailable: () => workspace().tabs.length > 1,
      run: () => {
        switchTab(1);
      },
    },
    {
      id: 'tab.previous',
      category: 'tab',
      titleKey: 'command.tab.previous',
      isAvailable: () => workspace().tabs.length > 1,
      run: () => {
        switchTab(-1);
      },
    },
    {
      id: 'tab.moveLeft',
      category: 'tab',
      titleKey: 'command.tab.moveLeft',
      isAvailable: () => activeIndex() > 0,
      run: () => {
        const { active } = workspace();
        if (active) {
          workspace().moveTab(active, activeIndex() - 1);
        }
      },
    },
    {
      id: 'tab.moveRight',
      category: 'tab',
      titleKey: 'command.tab.moveRight',
      isAvailable: () => activeIndex() >= 0 && activeIndex() < workspace().tabs.length - 1,
      run: () => {
        const { active } = workspace();
        if (active) {
          workspace().moveTab(active, activeIndex() + 1);
        }
      },
    },
    {
      id: 'palette.show',
      category: 'view',
      titleKey: 'command.palette.show',
      isAvailable: () => !ui().paletteOpen && ui().settingsSection === null,
      run: () => {
        ui().setPaletteOpen(true);
      },
    },
    {
      id: 'settings.open',
      category: 'app',
      titleKey: 'command.settings.open',
      isAvailable: () => ui().state !== null,
      run: () => {
        ui().openSettings('general');
      },
    },
    {
      id: 'settings.shortcuts',
      category: 'app',
      titleKey: 'command.settings.shortcuts',
      isAvailable: () => ui().state !== null,
      run: () => {
        ui().openSettings('shortcuts');
      },
    },
    {
      id: 'app.quit',
      category: 'app',
      titleKey: 'command.app.quit',
      isAvailable: () => true,
      run: requestQuit,
    },
    {
      id: 'sidebar.toggle',
      category: 'view',
      titleKey: 'command.sidebar.toggle',
      isAvailable: () => libraryOpen() && settings() !== null && !ui().focusMode,
      run: async () => {
        const current = settings();
        if (current) {
          await ui().update({ sidebar: { ...current.sidebar, visible: !current.sidebar.visible } });
        }
      },
    },
    {
      id: 'explorer.focus',
      category: 'view',
      titleKey: 'command.explorer.focus',
      isAvailable: () => libraryOpen() && settings() !== null && !ui().focusMode,
      run: async () => {
        const current = settings();
        if (current && !current.sidebar.visible) {
          await ui().update({ sidebar: { ...current.sidebar, visible: true } });
        }
        focusExplorer();
      },
    },
    {
      id: 'editor.focus',
      category: 'view',
      titleKey: 'command.editor.focus',
      isAvailable: () => workspace().active !== null,
      run: focusEditor,
    },
    {
      id: 'focus.toggle',
      category: 'view',
      titleKey: 'command.focus.toggle',
      isAvailable: libraryOpen,
      run: async () => {
        await ui().setFocusMode(!ui().focusMode);
        focusEditor();
      },
    },
    {
      id: 'zoom.in',
      category: 'view',
      titleKey: 'command.zoom.in',
      isAvailable: () => (settings()?.zoom ?? 2) < 2,
      run: async () => {
        await ui().update({ zoom: nextZoom(settings()?.zoom ?? 1, 1) });
      },
    },
    {
      id: 'zoom.out',
      category: 'view',
      titleKey: 'command.zoom.out',
      isAvailable: () => (settings()?.zoom ?? 0.5) > 0.5,
      run: async () => {
        await ui().update({ zoom: nextZoom(settings()?.zoom ?? 1, -1) });
      },
    },
    {
      id: 'zoom.reset',
      category: 'view',
      titleKey: 'command.zoom.reset',
      isAvailable: () => Math.abs((settings()?.zoom ?? 1) - 1) > 1e-9,
      run: async () => {
        await ui().update({ zoom: 1 });
      },
    },
    {
      id: 'theme.system',
      category: 'view',
      titleKey: 'command.theme.system',
      isAvailable: () => settings() !== null && settings()?.theme !== 'system',
      run: setTheme('system'),
    },
    {
      id: 'theme.light',
      category: 'view',
      titleKey: 'command.theme.light',
      isAvailable: () => settings() !== null && settings()?.theme !== 'light',
      run: setTheme('light'),
    },
    {
      id: 'theme.dark',
      category: 'view',
      titleKey: 'command.theme.dark',
      isAvailable: () => settings() !== null && settings()?.theme !== 'dark',
      run: setTheme('dark'),
    },
    {
      id: 'lineWidth.toggle',
      category: 'view',
      titleKey: 'command.lineWidth.toggle',
      isAvailable: () => settings() !== null,
      run: async () => {
        const current = settings();
        if (current) {
          await ui().update({ limitLineWidth: !current.limitLineWidth });
        }
      },
    },
  ];
}
