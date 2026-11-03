/**
 * Explorer (sections 11 and 13): the documents of the library. Subnets,
 * favourites and the other sections of section 13 follow with their phases;
 * Phase 3 lists the documents by title.
 *
 * The list is virtualised ("Lange Listen werden virtualisiert"): only the
 * visible rows exist in the DOM. It is one tab stop (ARIA listbox with
 * `aria-activedescendant`): arrow keys move, Enter opens, F2 renames, the
 * context menu key or Shift+F10 opens the context menu.
 */
import { FilePlus, FileText, Hash } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import {
  ContextMenu,
  isContextMenuKey,
  positionBelow,
  type ContextMenuState,
} from '@/components/ContextMenu';
import { useLocale, useT } from '@/i18n';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import { useWorkspace } from './workspaceStore';

export const EXPLORER_LIST_ID = 'pw-explorer-list';
const ROW_HEIGHT = 26;
const OVERSCAN = 8;

interface ExplorerProps {
  onNewDocument: () => void;
  newDocumentShortcut?: string | undefined;
  renameShortcut?: string | undefined;
}

export function Explorer({ onNewDocument, newDocumentShortcut, renameShortcut }: ExplorerProps) {
  const t = useT();
  const locale = useLocale((state) => state.locale);
  const documents = useWorkspace((state) => state.documents);
  const listLoaded = useWorkspace((state) => state.listLoaded);
  const active = useWorkspace((state) => state.active);
  const renaming = useWorkspace((state) => state.renaming);
  const openDocument = useWorkspace((state) => state.openDocument);
  const setRenaming = useWorkspace((state) => state.setRenaming);
  const rename = useWorkspace((state) => state.rename);

  const sorted = useMemo(() => {
    const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
    return [...documents].sort((a, b) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id));
  }, [documents, locale]);

  const [selected, setSelected] = useState<string | null>(active);
  const [followed, setFollowed] = useState(active);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState({ top: 0, height: 400 });

  // Follow the active tab, so the Explorer shows where one is.
  if (active !== followed) {
    setFollowed(active);
    if (active) {
      setSelected(active);
    }
  }

  const selectedIndex = sorted.findIndex((document) => document.id === selected);

  // Keep the selected row visible (and therefore rendered).
  useEffect(() => {
    const element = list.current;
    if (!element || selectedIndex < 0) {
      return;
    }
    const top = selectedIndex * ROW_HEIGHT;
    if (top < element.scrollTop) {
      element.scrollTop = top;
    } else if (top + ROW_HEIGHT > element.scrollTop + element.clientHeight) {
      element.scrollTop = top + ROW_HEIGHT - element.clientHeight;
    }
  }, [selectedIndex]);

  useEffect(() => {
    const element = list.current;
    if (!element) {
      return;
    }
    const update = () => {
      setScroll({ top: element.scrollTop, height: element.clientHeight || 400 });
    };
    update();
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null;
    observer?.observe(element);
    return () => {
      observer?.disconnect();
    };
  }, []);

  const first = Math.max(0, Math.floor(scroll.top / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(sorted.length, Math.ceil((scroll.top + scroll.height) / ROW_HEIGHT) + OVERSCAN);
  const visible = sorted.slice(first, last);

  const select = (index: number) => {
    const document = sorted[Math.max(0, Math.min(sorted.length - 1, index))];
    if (document) {
      setSelected(document.id);
    }
  };

  const openMenu = (document: DocumentSummary, position: { x: number; y: number }) => {
    setSelected(document.id);
    setMenu({
      ...position,
      label: document.title,
      items: [
        { id: 'open', label: t('explorer.open'), onSelect: () => void openDocument(document.id) },
        {
          id: 'rename',
          label: t('explorer.rename'),
          shortcut: renameShortcut,
          onSelect: () => {
            setRenaming(document.id);
          },
        },
      ],
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (renaming) {
      return;
    }
    const page = Math.max(1, Math.floor(scroll.height / ROW_HEIGHT) - 1);
    const current = selectedIndex < 0 ? -1 : selectedIndex;
    const document = sorted[current];
    if (isContextMenuKey(event)) {
      if (document) {
        const row = window.document.getElementById(optionId(document.id));
        openMenu(document, row ? positionBelow(row) : { x: 40, y: 80 });
      }
      event.preventDefault();
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        select(current + 1);
        break;
      case 'ArrowUp':
        select(current <= 0 ? 0 : current - 1);
        break;
      case 'Home':
        select(0);
        break;
      case 'End':
        select(sorted.length - 1);
        break;
      case 'PageDown':
        select(current + page);
        break;
      case 'PageUp':
        select(current - page);
        break;
      case 'Enter':
        if (document) {
          void openDocument(document.id);
        }
        break;
      case 'F2':
        if (document) {
          setRenaming(document.id);
        }
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  };

  return (
    <section aria-labelledby="explorer-title" className="flex h-full min-h-0 flex-col" data-testid="explorer">
      <header className="flex h-9 shrink-0 items-center justify-between px-3">
        <h2 id="explorer-title" className="text-ui font-semibold tracking-wide text-muted uppercase">
          {t('explorer.title')}
        </h2>
        <button
          type="button"
          className="inline-flex size-7 items-center justify-center rounded-pw text-fg hover:bg-surface"
          aria-label={t('command.document.new')}
          title={
            newDocumentShortcut
              ? `${t('command.document.new')} (${newDocumentShortcut})`
              : t('command.document.new')
          }
          data-testid="explorer-new"
          onClick={onNewDocument}
        >
          <FilePlus aria-hidden="true" className="size-4" />
        </button>
      </header>
      <div
        ref={list}
        id={EXPLORER_LIST_ID}
        role="listbox"
        tabIndex={0}
        aria-label={t('explorer.documents')}
        aria-activedescendant={selected && selectedIndex >= 0 ? optionId(selected) : undefined}
        className="relative min-h-0 flex-1 overflow-y-auto focus-visible:outline-offset-[-2px]"
        onScroll={(event) => {
          setScroll({ top: event.currentTarget.scrollTop, height: event.currentTarget.clientHeight || 400 });
        }}
        onKeyDown={onKeyDown}
        onFocus={() => {
          if (selectedIndex < 0) {
            select(0);
          }
        }}
      >
        <div style={{ height: sorted.length * ROW_HEIGHT }} className="relative">
          {visible.map((document, offset) => {
            const index = first + offset;
            const isSelected = document.id === selected;
            return (
              <div
                key={document.id}
                id={optionId(document.id)}
                role="option"
                aria-selected={isSelected}
                data-document={document.id}
                title={document.originalName ?? undefined}
                className={`text-ui absolute inset-x-0 flex cursor-default items-center gap-2 px-3 ${
                  isSelected ? 'bg-surface text-fg' : 'text-fg'
                } ${document.id === active ? 'font-semibold' : ''}`}
                style={{ top: index * ROW_HEIGHT, height: ROW_HEIGHT }}
                onClick={() => {
                  setSelected(document.id);
                  void openDocument(document.id);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  openMenu(document, { x: event.clientX, y: event.clientY });
                }}
              >
                {document.format === 'md' ? (
                  <Hash aria-hidden="true" className="size-3.5 shrink-0 text-muted" />
                ) : (
                  <FileText aria-hidden="true" className="size-3.5 shrink-0 text-muted" />
                )}
                {renaming === document.id ? (
                  <RenameInput
                    initial={document.title}
                    label={t('explorer.renameLabel')}
                    onDone={(title) => {
                      if (title === null || title === document.title) {
                        setRenaming(null);
                      } else {
                        void rename(document.id, title);
                      }
                      list.current?.focus();
                    }}
                  />
                ) : (
                  <span className="truncate">{document.title}</span>
                )}
              </div>
            );
          })}
        </div>
        {listLoaded && sorted.length === 0 ? (
          <p className="text-ui px-3 py-2 text-muted" data-testid="explorer-empty">
            {t('explorer.empty')}
          </p>
        ) : null}
      </div>
      {menu ? (
        <ContextMenu
          {...menu}
          onClose={() => {
            setMenu(null);
          }}
        />
      ) : null}
    </section>
  );
}

function optionId(id: string): string {
  return `pw-explorer-${id}`;
}

interface RenameInputProps {
  initial: string;
  label: string;
  /** The new title, or `null` if cancelled. */
  onDone: (title: string | null) => void;
}

function RenameInput({ initial, label, onDone }: RenameInputProps) {
  const [value, setValue] = useState(initial);
  const done = useRef(false);
  const finish = (title: string | null) => {
    if (!done.current) {
      done.current = true;
      onDone(title);
    }
  };
  return (
    <input
      // Renaming starts from the keyboard or the menu: the field takes the focus.
      autoFocus
      aria-label={label}
      data-testid="explorer-rename"
      className="text-ui h-5 min-w-0 flex-1 rounded-pw border border-accent bg-surface px-1 text-fg outline-none"
      value={value}
      maxLength={300}
      onFocus={(event) => {
        event.currentTarget.select();
      }}
      onChange={(event) => {
        setValue(event.target.value);
      }}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === 'Enter') {
          event.preventDefault();
          finish(value.trim() === '' ? null : value);
        } else if (event.key === 'Escape') {
          event.preventDefault();
          finish(null);
        }
      }}
      onBlur={() => {
        finish(value.trim() === '' ? null : value);
      }}
    />
  );
}
