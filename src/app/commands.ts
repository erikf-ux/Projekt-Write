/**
 * The commands of Phase 3 (ADR 0012). `isAvailable` decides what the
 * command palette shows and which menu items are enabled: only what works
 * right now (section 25, Phase 3: "Die Command Palette zeigt nur
 * funktionierende Befehle").
 */
import { translate, useLocale } from '@/i18n';
import type { CommandDefinition } from '@/modules/commands/types';
import { copyTitle } from '@/modules/documents/Explorer';
import {
  activeDocument,
  freeTitle,
  hasUnsavedChanges,
  useWorkspace,
} from '@/modules/documents/workspaceStore';
import { useFind } from '@/modules/editor/find/findStore';
import { getSession } from '@/modules/editor/session';
import { useStatistics } from '@/modules/editor/statisticsStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { nextZoom, useUi } from '@/modules/settings/uiStore';
import type { TextFormat } from '@/types/generated/TextFormat';
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

async function newDocument(format: TextFormat = 'md'): Promise<void> {
  const title = freeTitle(translate(useLocale.getState().locale, 'document.untitled'), workspace().documents);
  if (await workspace().createDocument(title, format)) {
    focusEditor();
  }
}

const text = (key: Parameters<typeof translate>[1]) => translate(useLocale.getState().locale, key);

/** The active tab, if it shows a library document. */
const activeLibraryDocument = () => {
  const document = activeDocument(workspace());
  return document && !document.external ? document : null;
};

/** The active tab, if it shows an external file. */
const activeExternal = () => {
  const document = activeDocument(workspace());
  return document?.external && document.loaded ? document : null;
};

const activeSession = () => {
  const id = workspace().active;
  return id ? getSession(id) : undefined;
};

const findOpen = () => {
  const id = workspace().active;
  return id !== null && useFind.getState().bars[id] !== undefined;
};

/** Opens the find bar; a short selection on one line becomes the search term. */
function openFind(replace: boolean): void {
  const id = workspace().active;
  const session = activeSession();
  if (!id || !session) {
    return;
  }
  const { from, to, empty } = session.editor.state.selection;
  const selected = empty ? '' : session.editor.state.doc.textBetween(from, to, '\n');
  const term = selected !== '' && selected.length <= 200 && !selected.includes('\n') ? selected : undefined;
  useFind.getState().open(id, replace, term);
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
      run: () => newDocument(),
    },
    {
      id: 'document.newRichText',
      category: 'document',
      titleKey: 'command.document.newRichText',
      isAvailable: libraryOpen,
      run: () => newDocument('pwrite'),
    },
    {
      id: 'document.openFile',
      category: 'document',
      titleKey: 'command.document.openFile',
      isAvailable: () => true,
      run: async () => {
        if (await workspace().openFile(text('external.dialog.open'))) {
          focusEditor();
        }
      },
    },
    {
      id: 'document.saveAs',
      category: 'document',
      titleKey: 'command.document.saveAs',
      isAvailable: () => {
        const document = activeExternal();
        return document !== null && !document.readOnly;
      },
      run: async () => {
        const document = activeExternal();
        if (document) {
          await workspace().saveAs(document.id, text('external.dialog.saveAs'));
        }
      },
    },
    {
      id: 'document.takeIntoLibrary',
      category: 'document',
      titleKey: 'command.document.takeIntoLibrary',
      isAvailable: () => libraryOpen() && activeExternal() !== null,
      run: async () => {
        const document = activeExternal();
        if (document) {
          await workspace().takeIntoLibrary(document.id);
        }
      },
    },
    {
      id: 'document.save',
      category: 'document',
      titleKey: 'command.document.save',
      isAvailable: () => {
        const document = activeDocument(workspace());
        if (!document) {
          return false;
        }
        if (document.external) {
          return document.state === 'error' || hasUnsavedChanges(document);
        }
        return document.state === 'pending' || document.state === 'error';
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
      isAvailable: () => activeLibraryDocument() !== null && !ui().focusMode,
      run: async () => {
        const current = settings();
        if (current && !current.sidebar.visible) {
          await ui().update({ sidebar: { ...current.sidebar, visible: true } });
        }
        workspace().setRenaming(workspace().active);
      },
    },
    {
      id: 'document.copy',
      category: 'document',
      titleKey: 'command.document.copy',
      isAvailable: () => activeLibraryDocument() !== null,
      run: async () => {
        const document = activeLibraryDocument();
        if (document) {
          const suffix = translate(useLocale.getState().locale, 'explorer.copySuffix');
          await workspace().copyDocument(document.id, copyTitle(document.title, suffix));
        }
      },
    },
    {
      id: 'document.toggleFavorite',
      category: 'document',
      titleKey: 'command.document.toggleFavorite',
      isAvailable: () => activeLibraryDocument() !== null,
      run: async () => {
        const { active, documents } = workspace();
        const summary = documents.find((entry) => entry.id === active);
        if (summary) {
          await workspace().setFavorite(summary.id, summary.favoritedAt === null);
        }
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
      id: 'editor.toggleSource',
      category: 'view',
      titleKey: 'command.editor.toggleSource',
      isAvailable: () => {
        const document = activeDocument(workspace());
        return document?.loaded === true && document.format === 'md' && libraryOpen();
      },
      run: () => {
        const id = workspace().active;
        if (id) {
          workspace().toggleView(id);
          focusEditor();
        }
      },
    },
    {
      id: 'find.show',
      category: 'edit',
      titleKey: 'command.find.show',
      isAvailable: () => activeDocument(workspace())?.loaded === true,
      run: () => {
        openFind(false);
      },
    },
    {
      id: 'replace.show',
      category: 'edit',
      titleKey: 'command.replace.show',
      isAvailable: () => activeDocument(workspace())?.loaded === true && libraryOpen(),
      run: () => {
        openFind(true);
      },
    },
    {
      id: 'find.next',
      category: 'edit',
      titleKey: 'command.find.next',
      isAvailable: () => findOpen(),
      run: () => {
        activeSession()?.findNext();
      },
    },
    {
      id: 'find.previous',
      category: 'edit',
      titleKey: 'command.find.previous',
      isAvailable: () => findOpen(),
      run: () => {
        activeSession()?.findPrevious();
      },
    },
    {
      id: 'statistics.show',
      category: 'view',
      titleKey: 'command.statistics.show',
      isAvailable: () => activeDocument(workspace())?.loaded === true,
      run: () => {
        useStatistics.getState().showDialog(true);
      },
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
