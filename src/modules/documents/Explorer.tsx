/**
 * Explorer (sections 11 and 13): a tree of the library's documents with the
 * entries "Favoriten", "Zuletzt verwendet" and "Alle Dokumente". Subnets
 * join the tree with Phase 7; an element in several entries stays one
 * element. "Zuletzt verwendet" also lists external files opened last
 * (ADR 0015, decision 10); one that is gone is marked and can be removed.
 *
 * The tree is virtualised ("Lange Listen werden virtualisiert"): only the
 * visible rows exist in the DOM, as a flat list with `aria-level`. It is one
 * tab stop (ARIA tree with `aria-activedescendant`): arrow keys move, Right
 * and Left open and close an entry, Enter opens, F2 renames, the context
 * menu key or Shift+F10 opens the context menu.
 */
import { ChevronDown, ChevronRight, FilePlus, FileText, HardDrive, Hash, PenLine, Star } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import {
  ContextMenu,
  isContextMenuKey,
  positionBelow,
  type ContextMenuState,
} from '@/components/ContextMenu';
import { useLocale, useT } from '@/i18n';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { RecentFileEntry } from '@/types/generated/RecentFileEntry';
import { useWorkspace } from './workspaceStore';

export const EXPLORER_LIST_ID = 'pw-explorer-list';
const ROW_HEIGHT = 26;
const OVERSCAN = 8;
/** Entries of "Zuletzt verwendet". */
export const RECENT_LIMIT = 10;
const MAX_TITLE = 300;

export type ExplorerGroup = 'favorites' | 'recent' | 'all';
const GROUPS: readonly ExplorerGroup[] = ['favorites', 'recent', 'all'];

type Row =
  | { kind: 'group'; key: string; group: ExplorerGroup; count: number; expanded: boolean }
  | {
      kind: 'document';
      key: string;
      group: ExplorerGroup;
      document: DocumentSummary;
      position: number;
      size: number;
    }
  | {
      kind: 'file';
      key: string;
      group: 'recent';
      file: RecentFileEntry;
      position: number;
      size: number;
    };

/** An entry of "Zuletzt verwendet": a library document or an external file. */
export type RecentEntry =
  { kind: 'document'; document: DocumentSummary } | { kind: 'file'; file: RecentFileEntry };

/** Library documents and external files of "Zuletzt verwendet", newest first. */
export function recentEntries(
  documents: readonly DocumentSummary[],
  files: readonly RecentFileEntry[],
): RecentEntry[] {
  const entries: { at: string; entry: RecentEntry }[] = [
    ...documents.map((document) => ({
      at: document.lastOpenedAt ?? '',
      entry: { kind: 'document', document } as const,
    })),
    ...files.map((file) => ({ at: file.openedAt, entry: { kind: 'file', file } as const })),
  ];
  return entries
    .sort((a, b) => b.at.localeCompare(a.at))
    .slice(0, RECENT_LIMIT)
    .map(({ entry }) => entry);
}

function FormatIcon({ format }: { format: DocumentFormat }) {
  if (format === 'md') {
    return <Hash aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
  }
  if (format === 'pwrite') {
    return <PenLine aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
  }
  return <FileText aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
}

/** The documents of each entry: favourites and all by title, recent by last opening. */
export function explorerGroups(
  documents: readonly DocumentSummary[],
  locale: string,
): Record<ExplorerGroup, DocumentSummary[]> {
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  const byTitle = [...documents].sort(
    (a, b) => collator.compare(a.title, b.title) || a.id.localeCompare(b.id),
  );
  const recent = documents
    .filter((document) => document.lastOpenedAt !== null)
    .sort((a, b) => (b.lastOpenedAt ?? '').localeCompare(a.lastOpenedAt ?? '') || a.id.localeCompare(b.id))
    .slice(0, RECENT_LIMIT);
  return {
    favorites: byTitle.filter((document) => document.favoritedAt !== null),
    recent,
    all: byTitle,
  };
}

/** Title of a copy: "Titel (Kopie)", shortened to fit. */
export function copyTitle(title: string, suffix: string): string {
  const room = MAX_TITLE - suffix.length;
  return `${title.length > room ? title.slice(0, room).trimEnd() : title}${suffix}`;
}

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
  const copyDocument = useWorkspace((state) => state.copyDocument);
  const setFavorite = useWorkspace((state) => state.setFavorite);
  const recentFiles = useWorkspace((state) => state.recentFiles);
  const openRecentFile = useWorkspace((state) => state.openRecentFile);
  const forgetRecentFile = useWorkspace((state) => state.forgetRecentFile);

  const [collapsed, setCollapsed] = useState<ReadonlySet<ExplorerGroup>>(() => new Set());
  const groups = useMemo(() => explorerGroups(documents, locale), [documents, locale]);
  const recent = useMemo(() => recentEntries(groups.recent, recentFiles), [groups.recent, recentFiles]);
  const rows = useMemo(() => {
    const result: Row[] = [];
    for (const group of GROUPS) {
      const members: RecentEntry[] =
        group === 'recent' ? recent : groups[group].map((document) => ({ kind: 'document', document }));
      const expanded = !collapsed.has(group) && members.length > 0;
      result.push({ kind: 'group', key: group, group, count: members.length, expanded });
      if (expanded) {
        members.forEach((entry, index) => {
          const position = index + 1;
          const size = members.length;
          result.push(
            entry.kind === 'document'
              ? {
                  kind: 'document',
                  key: `${group}:${entry.document.id}`,
                  group,
                  document: entry.document,
                  position,
                  size,
                }
              : {
                  kind: 'file',
                  key: `recent:file-${entry.file.id}`,
                  group: 'recent',
                  file: entry.file,
                  position,
                  size,
                },
          );
        });
      }
    }
    return result;
  }, [groups, recent, collapsed]);

  const [selected, setSelected] = useState<string | null>(active ? `all:${active}` : null);
  const [followed, setFollowed] = useState(active);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState({ top: 0, height: 400 });

  // Follow the active tab, so the Explorer shows where one is.
  if (active !== followed) {
    setFollowed(active);
    if (active) {
      const row =
        rows.find(
          (entry) => entry.kind === 'document' && entry.document.id === active && entry.group === 'all',
        ) ?? rows.find((entry) => entry.kind === 'document' && entry.document.id === active);
      if (row) {
        setSelected(row.key);
      }
    }
  }

  // Renaming (F2, menu, palette) happens in the selected row of the document.
  const [renamed, setRenamed] = useState(renaming);
  if (renaming !== renamed) {
    setRenamed(renaming);
    const shown = rows.find((row) => row.key === selected);
    if (renaming && !(shown?.kind === 'document' && shown.document.id === renaming)) {
      if (collapsed.has('all')) {
        setCollapsed((previous) => new Set([...previous].filter((group) => group !== 'all')));
      }
      setSelected(`all:${renaming}`);
    }
  }

  const selectedIndex = rows.findIndex((row) => row.key === selected);
  const current = rows[selectedIndex];

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
  const last = Math.min(rows.length, Math.ceil((scroll.top + scroll.height) / ROW_HEIGHT) + OVERSCAN);
  const visible = rows.slice(first, last);

  const select = (index: number) => {
    const row = rows[Math.max(0, Math.min(rows.length - 1, index))];
    if (row) {
      setSelected(row.key);
    }
  };

  const toggle = (group: ExplorerGroup, open?: boolean) => {
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (open ?? next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return next;
    });
  };

  const openMenu = (row: Extract<Row, { kind: 'document' | 'file' }>, position: { x: number; y: number }) => {
    setSelected(row.key);
    if (row.kind === 'file') {
      const { file } = row;
      setMenu({
        ...position,
        label: file.name,
        items: [
          { id: 'open', label: t('explorer.open'), onSelect: () => void openRecentFile(file.id, file.name) },
          { id: 'forget', label: t('explorer.forgetRecent'), onSelect: () => void forgetRecentFile(file.id) },
        ],
      });
      return;
    }
    const { document } = row;
    const favorite = document.favoritedAt !== null;
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
        {
          id: 'copy',
          label: t('explorer.copy'),
          onSelect: () => void copyDocument(document.id, copyTitle(document.title, t('explorer.copySuffix'))),
        },
        {
          id: 'favorite',
          label: favorite ? t('explorer.unfavorite') : t('explorer.favorite'),
          onSelect: () => void setFavorite(document.id, !favorite),
        },
      ],
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (renaming) {
      return;
    }
    const page = Math.max(1, Math.floor(scroll.height / ROW_HEIGHT) - 1);
    const index = selectedIndex < 0 ? -1 : selectedIndex;
    if (isContextMenuKey(event)) {
      if (current?.kind === 'document' || current?.kind === 'file') {
        const element = window.document.getElementById(rowId(current.key));
        openMenu(current, element ? positionBelow(element) : { x: 40, y: 80 });
      }
      event.preventDefault();
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        select(index + 1);
        break;
      case 'ArrowUp':
        select(index <= 0 ? 0 : index - 1);
        break;
      case 'ArrowRight':
        if (current?.kind === 'group') {
          if (current.expanded) {
            select(index + 1);
          } else if (current.count > 0) {
            toggle(current.group, true);
          }
        }
        break;
      case 'ArrowLeft':
        if (current?.kind === 'group' && current.expanded) {
          toggle(current.group, false);
        } else if (current?.kind === 'document' || current?.kind === 'file') {
          setSelected(current.group);
        }
        break;
      case 'Home':
        select(0);
        break;
      case 'End':
        select(rows.length - 1);
        break;
      case 'PageDown':
        select(index + page);
        break;
      case 'PageUp':
        select(index - page);
        break;
      case 'Enter':
        if (current?.kind === 'document') {
          void openDocument(current.document.id);
        } else if (current?.kind === 'file') {
          void openRecentFile(current.file.id, current.file.name);
        } else if (current?.kind === 'group' && current.count > 0) {
          toggle(current.group);
        }
        break;
      case 'F2':
        if (current?.kind === 'document') {
          setRenaming(current.document.id);
        }
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  };

  const groupLabel = (group: ExplorerGroup) => t(`explorer.group.${group}`);

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
        role="tree"
        tabIndex={0}
        aria-label={t('explorer.documents')}
        aria-activedescendant={current ? rowId(current.key) : undefined}
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
        <div style={{ height: rows.length * ROW_HEIGHT }} className="relative">
          {visible.map((row, offset) => {
            const index = first + offset;
            const isSelected = row.key === selected;
            const style = { top: index * ROW_HEIGHT, height: ROW_HEIGHT };
            const base = `text-ui absolute inset-x-0 flex cursor-default items-center gap-1.5 ${
              isSelected ? 'bg-surface text-fg' : 'text-fg'
            }`;
            if (row.kind === 'group') {
              const Chevron = row.expanded ? ChevronDown : ChevronRight;
              return (
                <div
                  key={row.key}
                  id={rowId(row.key)}
                  role="treeitem"
                  aria-level={1}
                  aria-setsize={GROUPS.length}
                  aria-posinset={GROUPS.indexOf(row.group) + 1}
                  aria-expanded={row.count > 0 ? row.expanded : undefined}
                  aria-selected={isSelected}
                  data-group={row.group}
                  className={`${base} px-1.5 font-semibold`}
                  style={style}
                  onClick={() => {
                    setSelected(row.key);
                    if (row.count > 0) {
                      toggle(row.group);
                    }
                  }}
                >
                  <Chevron
                    aria-hidden="true"
                    className={`size-3.5 shrink-0 text-muted ${row.count > 0 ? '' : 'invisible'}`}
                  />
                  <span className="truncate">{groupLabel(row.group)}</span>
                  <span className="ml-auto pr-2 font-normal text-muted">{row.count}</span>
                </div>
              );
            }
            if (row.kind === 'file') {
              const { file } = row;
              return (
                <div
                  key={row.key}
                  id={rowId(row.key)}
                  role="treeitem"
                  aria-level={2}
                  aria-setsize={row.size}
                  aria-posinset={row.position}
                  aria-selected={isSelected}
                  data-recent-file={file.id}
                  data-group-member={row.group}
                  data-missing={file.missing ? true : undefined}
                  title={t('explorer.externalFile', { folder: file.folder })}
                  className={`${base} pr-3 pl-7 ${file.missing ? 'text-muted' : ''}`}
                  style={style}
                  onClick={() => {
                    setSelected(row.key);
                    void openRecentFile(file.id, file.name);
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    openMenu(row, { x: event.clientX, y: event.clientY });
                  }}
                >
                  <FormatIcon format={file.format} />
                  <span className={`truncate ${file.missing ? 'line-through' : ''}`}>{file.name}</span>
                  {file.missing ? <span className="sr-only">({t('explorer.fileMissing')})</span> : null}
                  <HardDrive
                    role="img"
                    aria-label={t('explorer.externalFile', { folder: file.folder })}
                    className="ml-auto size-3 shrink-0 text-muted"
                  />
                </div>
              );
            }
            const { document } = row;
            return (
              <div
                key={row.key}
                id={rowId(row.key)}
                role="treeitem"
                aria-level={2}
                aria-setsize={row.size}
                aria-posinset={row.position}
                aria-selected={isSelected}
                data-document={document.id}
                data-group-member={row.group}
                title={document.originalName ?? undefined}
                className={`${base} pr-3 pl-7 ${document.id === active ? 'font-semibold' : ''}`}
                style={style}
                onClick={() => {
                  setSelected(row.key);
                  void openDocument(document.id);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  openMenu(row, { x: event.clientX, y: event.clientY });
                }}
              >
                <FormatIcon format={document.format} />
                {renaming === document.id && isSelected ? (
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
                {document.favoritedAt !== null && row.group !== 'favorites' ? (
                  <Star
                    role="img"
                    aria-label={t('explorer.isFavorite')}
                    className="ml-auto size-3 shrink-0 text-muted"
                  />
                ) : null}
              </div>
            );
          })}
        </div>
        {listLoaded && documents.length === 0 ? (
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

function rowId(key: string): string {
  return `pw-explorer-${key.replace(':', '-')}`;
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
