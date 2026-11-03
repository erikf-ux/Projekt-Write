/**
 * Main layout (section 11): toolbar, activity bar, side bar with the
 * Explorer, the writing surface with document tabs, status bar. The native
 * menu bar sits above it. Focus mode hides everything except the writing
 * surface.
 */
import {
  Files,
  FilePlus,
  FileText,
  Maximize2,
  PanelLeft,
  Save,
  Settings,
  SquareTerminal,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { ActivityBar } from '@/components/navigation/ActivityBar';
import { Sidebar } from '@/components/layout/Sidebar';
import { StatusBar } from '@/components/layout/StatusBar';
import { TabBar, tabElementId } from '@/components/tabs/TabBar';
import { IconToolbar, type ToolbarItem } from '@/components/toolbar/IconToolbar';
import { useT } from '@/i18n';
import { findConflicts } from '@/modules/commands/keymap';
import { findCommand, runCommand } from '@/modules/commands/registry';
import type { CommandId } from '@/modules/commands/types';
import { Explorer } from '@/modules/documents/Explorer';
import { activeDocument, useWorkspace } from '@/modules/documents/workspaceStore';
import { DocumentEditor, panelElementId } from '@/modules/editor/DocumentEditor';
import { StatisticsDialog } from '@/modules/editor/StatisticsDialog';
import { readingMinutes } from '@/modules/editor/statistics';
import { useStatistics } from '@/modules/editor/statisticsStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { useUi } from '@/modules/settings/uiStore';
import { useBindings, useShortcutLabel } from './bindings';

interface WorkbenchProps {
  /** The first start just ended: open an empty document at once (section 11). */
  openEmptyDocument: boolean;
}

export function Workbench({ openEmptyDocument }: WorkbenchProps) {
  const t = useT();
  const shortcut = useShortcutLabel();
  const bindings = useBindings();
  const status = useLibrary((state) => state.status);
  const library = status?.state === 'open' ? status.library : null;
  const settings = useUi((state) => state.state?.settings);
  const focusMode = useUi((state) => state.focusMode);
  const update = useUi((state) => state.update);
  const openSettings = useUi((state) => state.openSettings);
  const workspace = useWorkspace();
  const current = activeDocument(workspace);
  const statistics = useStatistics((state) => (current ? state.byDocument[current.id] : undefined));
  const statisticsOpen = useStatistics((state) => state.dialogOpen);
  const [previewWidth, setPreviewWidth] = useState<number | null>(null);
  const conflicts = useMemo(() => findConflicts(bindings).length, [bindings]);
  const created = useRef(false);

  // A library was opened (or another one): load its documents.
  const libraryKey = library?.displayPath ?? null;
  useEffect(() => {
    if (libraryKey === null) {
      return;
    }
    void (async () => {
      await useWorkspace.getState().reset();
      await useWorkspace.getState().refresh();
      const { documents } = useWorkspace.getState();
      if (openEmptyDocument && !created.current && documents.length === 0) {
        created.current = true;
        await runCommand('document.new');
      }
    })();
  }, [libraryKey, openEmptyDocument]);

  // Save on focus loss of the window (section 9).
  useEffect(() => {
    const onBlur = () => {
      void useWorkspace.getState().flushAll();
    };
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  if (!settings) {
    return null;
  }

  const run = (id: CommandId) => () => void runCommand(id);
  const available = (id: CommandId) => findCommand(id)?.isAvailable() ?? false;
  const item = (id: CommandId, icon: ToolbarItem['icon'], extra: Partial<ToolbarItem> = {}): ToolbarItem => {
    const command = findCommand(id);
    return {
      id,
      icon,
      label: command ? t(command.titleKey) : id,
      shortcut: shortcut(id),
      disabled: !available(id),
      onSelect: run(id),
      ...extra,
    };
  };

  const sidebarVisible = settings.sidebar.visible && !focusMode;
  const width = previewWidth ?? settings.sidebar.width;
  const tabs = workspace.tabs.flatMap((id) => {
    const document = workspace.open[id];
    return document
      ? [
          {
            id,
            title: document.title,
            marker:
              document.state === 'error' || document.state === 'blocked' ? ('error' as const) : undefined,
          },
        ]
      : [];
  });

  return (
    <div
      className="flex h-full flex-col bg-surface text-fg"
      data-testid="workbench"
      data-focus-mode={focusMode}
    >
      {!focusMode ? (
        <header className="flex h-9 shrink-0 items-center border-b border-muted/40 bg-sidebar px-2">
          <IconToolbar
            label={t('toolbar.label')}
            orientation="horizontal"
            testId="toolbar"
            items={[
              item('sidebar.toggle', PanelLeft, { pressed: settings.sidebar.visible }),
              item('document.new', FilePlus),
              item('document.save', Save),
              item('palette.show', SquareTerminal),
              item('focus.toggle', Maximize2),
              item('settings.open', Settings),
            ]}
          />
        </header>
      ) : null}
      <div className="flex min-h-0 flex-1">
        {!focusMode ? (
          <ActivityBar
            label={t('activity.label')}
            items={[
              item('explorer.focus', Files, {
                label: t('activity.explorer'),
                pressed: settings.sidebar.visible,
              }),
              item('editor.focus', FileText, { label: t('activity.editor') }),
            ]}
            bottom={[item('settings.open', Settings)]}
          />
        ) : null}
        {sidebarVisible ? (
          <Sidebar
            width={width}
            onResize={(next, final) => {
              if (final) {
                setPreviewWidth(null);
                void update({ sidebar: { ...settings.sidebar, width: next } });
              } else {
                setPreviewWidth(next);
              }
            }}
          >
            <Explorer
              onNewDocument={run('document.new')}
              newDocumentShortcut={shortcut('document.new')}
              renameShortcut={shortcut('document.rename')}
            />
          </Sidebar>
        ) : null}
        <main className="flex min-w-0 flex-1 flex-col" aria-label={t('main.label')}>
          {!focusMode ? (
            <TabBar
              tabs={tabs}
              active={workspace.active}
              panelId={panelElementId}
              closeShortcut={shortcut('tab.close')}
              onActivate={(id) => {
                workspace.activate(id);
              }}
              onClose={(id) => void workspace.closeTab(id)}
              onCloseOthers={(id) => void workspace.closeOthers(id)}
              onCloseAll={() => void workspace.closeAll()}
              onMove={(id, index) => {
                workspace.moveTab(id, index);
              }}
            />
          ) : null}
          <DocumentEditor
            tabs={workspace.tabs}
            open={workspace.open}
            active={workspace.active}
            tabElementId={tabElementId}
            limitLineWidth={settings.limitLineWidth}
            focusMode={focusMode}
            readOnly={status?.state !== 'open'}
            onNewDocument={available('document.new') ? run('document.new') : null}
            newDocumentShortcut={shortcut('document.new')}
          />
        </main>
      </div>
      {!focusMode ? (
        <StatusBar
          library={library}
          saveState={current?.loaded ? current.state : null}
          format={current?.format ?? null}
          view={current?.loaded ? current.view : null}
          zoom={settings.zoom}
          conflicts={conflicts}
          statistics={
            current?.loaded && statistics
              ? {
                  words: statistics.document.words,
                  selectedWords: statistics.selection?.words ?? null,
                  minutes: settings.readingTime.visible
                    ? readingMinutes(
                        statistics.selection?.words ?? statistics.document.words,
                        settings.readingTime.wordsPerMinute,
                      )
                    : null,
                }
              : null
          }
          onShowStatistics={() => {
            useStatistics.getState().showDialog(true);
          }}
          onRetrySave={() => {
            if (current) {
              void workspace.save(current.id);
            }
          }}
          onResetZoom={() => void update({ zoom: 1 })}
          onShowConflicts={() => {
            openSettings('shortcuts');
          }}
        />
      ) : (
        <p className="sr-only" role="status">
          {t('focus.hint', { shortcut: shortcut('focus.toggle') ?? 'F11' })}
        </p>
      )}
      {statisticsOpen && statistics ? (
        <StatisticsDialog
          statistics={statistics}
          wordsPerMinute={settings.readingTime.visible ? settings.readingTime.wordsPerMinute : null}
          onClose={() => {
            useStatistics.getState().showDialog(false);
          }}
        />
      ) : null}
    </div>
  );
}
