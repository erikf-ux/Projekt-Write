/**
 * Document tabs (sections 11 and 12): one tab per open document.
 *
 * ARIA tabs pattern with a roving tab stop: Left/Right move between tabs,
 * Home/End jump, Enter or Space activates, Delete closes. Tabs can be
 * reordered with the keyboard (commands "Tab nach links/rechts
 * verschieben") and by drag and drop. Middle click closes.
 */
import { X } from 'lucide-react';
import { useRef, useState, type DragEvent, type KeyboardEvent } from 'react';
import {
  ContextMenu,
  isContextMenuKey,
  positionBelow,
  type ContextMenuState,
} from '@/components/ContextMenu';
import { useT } from '@/i18n';

export interface TabInfo {
  id: string;
  title: string;
  /** Visible marker next to the title: a failed save, or unsaved changes. */
  marker?: 'error' | 'unsaved' | undefined;
  /** Shown on hover instead of the title (external files: the folder). */
  tooltip?: string | undefined;
}

interface TabBarProps {
  tabs: readonly TabInfo[];
  active: string | null;
  panelId: (id: string) => string;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onCloseOthers: (id: string) => void;
  onCloseAll: () => void;
  onMove: (id: string, index: number) => void;
  closeShortcut?: string | undefined;
}

const DRAG_TYPE = 'application/x-project-write-tab';

export function tabElementId(id: string): string {
  return `pw-tab-${id}`;
}

export function TabBar({
  tabs,
  active,
  panelId,
  onActivate,
  onClose,
  onCloseOthers,
  onCloseAll,
  onMove,
  closeShortcut,
}: TabBarProps) {
  const t = useT();
  const [focused, setFocused] = useState<string | null>(null);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const stop = focused && tabs.some((tab) => tab.id === focused) ? focused : active;

  const focusTab = (index: number) => {
    const tab = tabs[(index + tabs.length) % tabs.length];
    if (tab) {
      setFocused(tab.id);
      document.getElementById(tabElementId(tab.id))?.focus();
    }
  };

  const openMenu = (tab: TabInfo, position: { x: number; y: number }) => {
    const index = tabs.findIndex((other) => other.id === tab.id);
    setMenu({
      ...position,
      label: tab.title,
      items: [
        {
          id: 'close',
          label: t('command.tab.close'),
          shortcut: closeShortcut,
          onSelect: () => {
            onClose(tab.id);
          },
        },
        ...(tabs.length > 1
          ? [
              {
                id: 'closeOthers',
                label: t('command.tab.closeOthers'),
                onSelect: () => {
                  onCloseOthers(tab.id);
                },
              },
            ]
          : []),
        { id: 'closeAll', label: t('command.tab.closeAll'), onSelect: onCloseAll },
        ...(index > 0
          ? [
              {
                id: 'moveLeft',
                label: t('command.tab.moveLeft'),
                onSelect: () => {
                  onMove(tab.id, index - 1);
                },
              },
            ]
          : []),
        ...(index < tabs.length - 1
          ? [
              {
                id: 'moveRight',
                label: t('command.tab.moveRight'),
                onSelect: () => {
                  onMove(tab.id, index + 1);
                },
              },
            ]
          : []),
      ],
    });
  };

  const onKeyDown = (event: KeyboardEvent, tab: TabInfo, index: number) => {
    if (isContextMenuKey(event)) {
      const element = document.getElementById(tabElementId(tab.id));
      openMenu(tab, element ? positionBelow(element) : { x: 80, y: 40 });
      event.preventDefault();
      return;
    }
    switch (event.key) {
      case 'ArrowRight':
        focusTab(index + 1);
        break;
      case 'ArrowLeft':
        focusTab(index - 1);
        break;
      case 'Home':
        focusTab(0);
        break;
      case 'End':
        focusTab(tabs.length - 1);
        break;
      case 'Enter':
      case ' ':
        onActivate(tab.id);
        break;
      case 'Delete':
        onClose(tab.id);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  /** Index a dragged tab would land at, from the pointer position. */
  const indexAt = (event: DragEvent): number => {
    const elements = [...(list.current?.querySelectorAll<HTMLElement>('[role="tab"]') ?? [])];
    const index = elements.findIndex((element) => {
      const rect = element.getBoundingClientRect();
      return event.clientX < rect.left + rect.width / 2;
    });
    return index === -1 ? tabs.length : index;
  };

  if (tabs.length === 0) {
    return null;
  }

  return (
    <div
      ref={list}
      role="tablist"
      aria-label={t('tabs.label')}
      data-testid="tabs"
      className="flex h-9 shrink-0 items-stretch overflow-x-auto border-b border-muted/40 bg-sidebar"
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes(DRAG_TYPE)) {
          event.preventDefault();
          event.dataTransfer.dropEffect = 'move';
          setDropIndex(indexAt(event));
        }
      }}
      onDragLeave={() => {
        setDropIndex(null);
      }}
      onDrop={(event) => {
        const id = event.dataTransfer.getData(DRAG_TYPE);
        setDropIndex(null);
        if (!id) {
          return;
        }
        event.preventDefault();
        const from = tabs.findIndex((tab) => tab.id === id);
        let to = indexAt(event);
        if (from !== -1 && from < to) {
          to -= 1;
        }
        onMove(id, to);
      }}
    >
      {tabs.map((tab, index) => {
        const selected = tab.id === active;
        return (
          <div
            key={tab.id}
            id={tabElementId(tab.id)}
            role="tab"
            aria-selected={selected}
            aria-controls={panelId(tab.id)}
            tabIndex={tab.id === stop ? 0 : -1}
            draggable
            data-tab={tab.id}
            data-unsaved={tab.marker === 'unsaved' ? true : undefined}
            title={tab.tooltip ?? tab.title}
            className={`text-ui group relative flex max-w-56 min-w-24 cursor-default items-center gap-2 border-r border-muted/40 pr-1 pl-3 focus-visible:outline-offset-[-2px] ${
              selected ? 'bg-surface text-fg' : 'text-muted'
            } ${dropIndex === index ? 'border-l-2 border-l-accent' : ''}`}
            onClick={() => {
              onActivate(tab.id);
            }}
            onAuxClick={(event) => {
              if (event.button === 1) {
                event.preventDefault();
                onClose(tab.id);
              }
            }}
            onFocus={() => {
              setFocused(tab.id);
            }}
            onKeyDown={(event) => {
              onKeyDown(event, tab, index);
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              openMenu(tab, { x: event.clientX, y: event.clientY });
            }}
            onDragStart={(event) => {
              event.dataTransfer.setData(DRAG_TYPE, tab.id);
              event.dataTransfer.effectAllowed = 'move';
            }}
          >
            {selected ? (
              <span aria-hidden="true" className="absolute inset-x-0 top-0 h-0.5 bg-accent" />
            ) : null}
            <span className="truncate">{tab.title}</span>
            {tab.marker === 'error' ? (
              <span className="text-danger" title={t('status.saveError')}>
                !<span className="sr-only">{t('status.saveError')}</span>
              </span>
            ) : tab.marker === 'unsaved' ? (
              <span className="text-accent" title={t('tab.unsaved')} data-testid="tab-unsaved">
                ●<span className="sr-only">{t('tab.unsaved')}</span>
              </span>
            ) : null}
            <button
              type="button"
              tabIndex={-1}
              aria-label={t('tabs.close', { title: tab.title })}
              title={closeShortcut ? `${t('command.tab.close')} (${closeShortcut})` : t('command.tab.close')}
              className="ml-auto inline-flex size-5 shrink-0 items-center justify-center rounded-pw text-muted hover:bg-sidebar hover:text-fg"
              onClick={(event) => {
                event.stopPropagation();
                onClose(tab.id);
              }}
            >
              <X aria-hidden="true" className="size-3.5" />
            </button>
          </div>
        );
      })}
      {menu ? (
        <ContextMenu
          {...menu}
          onClose={() => {
            setMenu(null);
          }}
        />
      ) : null}
    </div>
  );
}
