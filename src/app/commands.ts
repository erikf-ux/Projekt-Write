/**
 * The commands of Phase 3 (ADR 0012). `isAvailable` decides what the
 * command palette shows and which menu items are enabled: only what works
 * right now (section 25, Phase 3: "Die Command Palette zeigt nur
 * funktionierende Befehle").
 */
import { translate, useLocale } from '@/i18n';
import type { CommandDefinition } from '@/modules/commands/types';
import { copyTitle } from '@/modules/documents/Explorer';
import { useExplorer } from '@/modules/documents/explorerStore';
import {
  activeDocument,
  freeTitle,
  hasUnsavedChanges,
  useWorkspace,
} from '@/modules/documents/workspaceStore';
import { useFind } from '@/modules/editor/find/findStore';
import { useHistory } from '@/modules/history/HistoryDialog';
import { getSession } from '@/modules/editor/session';
import { useStatistics } from '@/modules/editor/statisticsStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { useMedia } from '@/modules/media/mediaStore';
import { nextZoom, useUi } from '@/modules/settings/uiStore';
import { useStructureDialogs } from '@/modules/structure/dialogStore';
import { useStructure } from '@/modules/structure/structureStore';
import { rebuildSearchIndex } from '@/modules/search/rebuild';
import { useBackup } from '@/modules/backup/backupStore';
import {
  exportActive,
  exportLibrary,
  exportPackActive,
  importPack,
  printActive,
} from '@/modules/export/exportActions';
import { useSearch } from '@/modules/search/searchStore';
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

/** The list entry of a library document. */
const activeSummary = (id: string) => workspace().documents.find((entry) => entry.id === id);

/** The active tab, if it shows an external file. */
const activeExternal = () => {
  const document = activeDocument(workspace());
  return document?.external && document.loaded ? document : null;
};

/** "In … umwandeln" for the active library document (ADR 0015, decision 13). */
function convertCommand(
  id: 'document.convertToText' | 'document.convertToMarkdown' | 'document.convertToRichText',
  titleKey:
    | 'command.document.convertToText'
    | 'command.document.convertToMarkdown'
    | 'command.document.convertToRichText',
  format: TextFormat,
): CommandDefinition {
  return {
    id,
    category: 'document',
    titleKey,
    isAvailable: () => {
      const document = activeLibraryDocument();
      return document !== null && document.loaded && !document.readOnly && document.format !== format;
    },
    run: async () => {
      const document = activeLibraryDocument();
      if (document) {
        await workspace().convertDocument(document.id, format);
      }
    },
  };
}

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
      id: 'document.print',
      category: 'document',
      titleKey: 'command.document.print',
      isAvailable: () => activeDocument(workspace())?.loaded === true,
      run: async () => {
        await printActive(false);
      },
    },
    {
      id: 'export.pdf',
      category: 'document',
      titleKey: 'command.export.pdf',
      isAvailable: () => activeDocument(workspace())?.loaded === true,
      run: async () => {
        await printActive(true);
      },
    },
    {
      id: 'export.txt',
      category: 'document',
      titleKey: 'command.export.txt',
      isAvailable: () => activeLibraryDocument()?.loaded === true,
      run: async () => {
        await exportActive('txt');
      },
    },
    {
      id: 'export.markdown',
      category: 'document',
      titleKey: 'command.export.markdown',
      isAvailable: () => activeLibraryDocument()?.loaded === true,
      run: async () => {
        await exportActive('markdown');
      },
    },
    {
      id: 'export.pack',
      category: 'document',
      titleKey: 'command.export.pack',
      isAvailable: () => activeLibraryDocument()?.loaded === true,
      run: async () => {
        await exportPackActive();
      },
    },
    {
      id: 'import.pack',
      category: 'document',
      titleKey: 'command.import.pack',
      isAvailable: libraryOpen,
      run: async () => {
        await importPack();
      },
    },
    {
      id: 'export.library',
      category: 'app',
      titleKey: 'command.export.library',
      isAvailable: libraryOpen,
      run: async () => {
        await exportLibrary();
      },
    },
    {
      id: 'backup.create',
      category: 'app',
      titleKey: 'command.backup.create',
      isAvailable: () => libraryOpen() && !useBackup.getState().busy,
      run: async () => {
        await useBackup.getState().create();
      },
    },
    {
      id: 'backup.check',
      category: 'app',
      titleKey: 'command.backup.check',
      isAvailable: () => !useBackup.getState().busy,
      run: async () => {
        await useBackup.getState().check();
      },
    },
    {
      id: 'backup.restore',
      category: 'app',
      titleKey: 'command.backup.restore',
      isAvailable: () => !useBackup.getState().busy,
      run: async () => {
        await useBackup.getState().restore();
      },
    },
    {
      id: 'document.delete',
      category: 'document',
      titleKey: 'command.document.delete',
      isAvailable: () => activeLibraryDocument() !== null,
      run: async () => {
        const document = activeLibraryDocument();
        if (document) {
          await workspace().deleteDocument(document.id);
        }
      },
    },
    {
      id: 'document.archive',
      category: 'document',
      titleKey: 'command.document.archive',
      isAvailable: () => {
        const document = activeLibraryDocument();
        return document !== null && activeSummary(document.id)?.archivedAt === null;
      },
      run: async () => {
        const document = activeLibraryDocument();
        if (document) {
          await workspace().archiveDocument(document.id, true);
        }
      },
    },
    {
      id: 'document.unarchive',
      category: 'document',
      titleKey: 'command.document.unarchive',
      isAvailable: () => {
        const document = activeLibraryDocument();
        return document !== null && typeof activeSummary(document.id)?.archivedAt === 'string';
      },
      run: async () => {
        const document = activeLibraryDocument();
        if (document) {
          await workspace().archiveDocument(document.id, false);
        }
      },
    },
    {
      id: 'document.history',
      category: 'document',
      titleKey: 'command.document.history',
      isAvailable: () => activeLibraryDocument()?.loaded === true,
      run: () => {
        const document = activeLibraryDocument();
        if (document) {
          useHistory.getState().open(document.id);
        }
      },
    },
    {
      id: 'document.saveVersion',
      category: 'document',
      titleKey: 'command.document.saveVersion',
      isAvailable: () => {
        const document = activeLibraryDocument();
        return document?.loaded === true && !document.readOnly;
      },
      run: () => {
        const document = activeLibraryDocument();
        if (document) {
          useHistory.getState().askName(document.id);
        }
      },
    },
    {
      id: 'subnet.new',
      category: 'document',
      titleKey: 'command.subnet.new',
      isAvailable: libraryOpen,
      run: () => {
        useStructureDialogs.getState().openSubnet({ mode: 'create', parentId: null });
      },
    },
    {
      id: 'document.moveTo',
      category: 'document',
      titleKey: 'command.document.moveTo',
      isAvailable: () =>
        activeLibraryDocument() !== null &&
        activeSummary(activeLibraryDocument()?.id ?? '')?.archivedAt === null,
      run: () => {
        const document = activeLibraryDocument();
        if (document) {
          useStructureDialogs.getState().openPicker({ mode: 'moveDocument', documentId: document.id });
        }
      },
    },
    {
      id: 'document.assignTo',
      category: 'document',
      titleKey: 'command.document.assignTo',
      isAvailable: () =>
        activeLibraryDocument() !== null &&
        activeSummary(activeLibraryDocument()?.id ?? '')?.archivedAt === null &&
        useStructure.getState().subnets.length > 0,
      run: () => {
        const document = activeLibraryDocument();
        if (document) {
          useStructureDialogs.getState().openPicker({ mode: 'assignDocument', documentId: document.id });
        }
      },
    },
    {
      id: 'document.tags',
      category: 'document',
      titleKey: 'command.document.tags',
      isAvailable: () => activeLibraryDocument() !== null,
      run: () => {
        const document = activeLibraryDocument();
        if (document) {
          useStructureDialogs.getState().openTags(document.id);
        }
      },
    },
    {
      id: 'tags.manage',
      category: 'document',
      titleKey: 'command.tags.manage',
      isAvailable: libraryOpen,
      run: () => {
        useStructureDialogs.getState().openTagManager();
      },
    },
    {
      id: 'explorer.filter',
      category: 'view',
      titleKey: 'command.explorer.filter',
      isAvailable: libraryOpen,
      run: () => {
        useExplorer.getState().showFilter(true);
        const sidebar = settings()?.sidebar;
        if (sidebar && !sidebar.visible) {
          void ui().update({ sidebar: { ...sidebar, visible: true } });
        }
        // The field exists once the Explorer has drawn the filter.
        requestAnimationFrame(() => {
          document.querySelector<HTMLInputElement>('[data-testid="explorer-filter-text"]')?.focus();
        });
      },
    },
    {
      id: 'structure.undo',
      category: 'edit',
      titleKey: 'command.structure.undo',
      isAvailable: () => libraryOpen() && useStructure.getState().history.undo !== null,
      run: () =>
        useStructure
          .getState()
          .undo()
          .then(() => undefined),
    },
    {
      id: 'structure.redo',
      category: 'edit',
      titleKey: 'command.structure.redo',
      isAvailable: () => libraryOpen() && useStructure.getState().history.redo !== null,
      run: () =>
        useStructure
          .getState()
          .redo()
          .then(() => undefined),
    },
    {
      id: 'media.import',
      category: 'document',
      titleKey: 'command.media.import',
      isAvailable: () => libraryOpen() && useMedia.getState().importing === null,
      run: () => useMedia.getState().startImport('files', text('media.import.title')),
    },
    {
      id: 'media.importFolder',
      category: 'document',
      titleKey: 'command.media.importFolder',
      isAvailable: () => libraryOpen() && useMedia.getState().importing === null,
      run: () => useMedia.getState().startImport('folder', text('media.import.titleFolder')),
    },
    {
      id: 'media.gallery',
      category: 'document',
      titleKey: 'command.media.gallery',
      isAvailable: libraryOpen,
      run: () => {
        workspace().openGallery();
      },
    },
    {
      id: 'mesh.open',
      category: 'view',
      titleKey: 'command.mesh.open',
      isAvailable: libraryOpen,
      run: () => {
        workspace().openMesh();
      },
    },
    {
      id: 'trash.empty',
      category: 'document',
      titleKey: 'command.trash.empty',
      isAvailable: () => libraryOpen() && workspace().trash.length > 0,
      run: () =>
        workspace()
          .emptyTrash()
          .then(() => undefined),
    },
    convertCommand('document.convertToText', 'command.document.convertToText', 'txt'),
    convertCommand('document.convertToMarkdown', 'command.document.convertToMarkdown', 'md'),
    convertCommand('document.convertToRichText', 'command.document.convertToRichText', 'pwrite'),
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
        ui().setSideView('explorer');
        focusExplorer();
      },
    },
    {
      id: 'search.focus',
      category: 'view',
      titleKey: 'command.search.focus',
      isAvailable: () => libraryOpen() && settings() !== null && !ui().focusMode,
      run: async () => {
        const current = settings();
        if (current && !current.sidebar.visible) {
          await ui().update({ sidebar: { ...current.sidebar, visible: true } });
        }
        ui().setSideView('search');
        useSearch.getState().requestFocus();
      },
    },
    {
      id: 'search.rebuild',
      category: 'app',
      titleKey: 'command.search.rebuild',
      isAvailable: libraryOpen,
      run: async () => {
        await rebuildSearchIndex();
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
