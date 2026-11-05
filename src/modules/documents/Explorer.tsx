/**
 * Explorer (sections 11, 13 and 16; ADR 0017): Favoriten, Zuletzt
 * verwendet, the subnets as a tree, Nicht zugeordnet, Archiv, Papierkorb.
 * A document in several subnets appears in each and stays one element; a
 * further membership carries a mark. "Zuletzt verwendet" also lists
 * external files (ADR 0015); "Archiv" and "Papierkorb" hold what is
 * archived or deleted (ADR 0016).
 *
 * The tree is virtualised ("Lange Listen werden virtualisiert"): only the
 * visible rows exist in the DOM, as a flat list with `aria-level`. It is one
 * tab stop (ARIA tree with `aria-activedescendant`): arrow keys move, Right
 * and Left open and close, Enter opens, F2 renames (a subnet: edit), `Entf`
 * deletes, the context menu key or Shift+F10 opens the menu, `Strg + Z` and
 * `Strg + Y` undo and redo structural steps. Drag & drop is the short way:
 * a document onto a subnet moves it (with `Strg` it is assigned as well),
 * onto "Nicht zugeordnet" it leaves every subnet; a subnet onto a subnet
 * becomes its child.
 */
import {
  ChevronDown,
  ChevronRight,
  FilePlus,
  FileText,
  Filter,
  HardDrive,
  Hash,
  Link2,
  Network,
  PenLine,
  Star,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type DragEvent, type KeyboardEvent } from 'react';
import {
  ContextMenu,
  isContextMenuKey,
  positionBelow,
  type ContextMenuItem,
  type ContextMenuState,
} from '@/components/ContextMenu';
import { useLocale, useT } from '@/i18n';
import { useUi } from '@/modules/settings/uiStore';
import { useStructureDialogs } from '@/modules/structure/dialogStore';
import { useStructure } from '@/modules/structure/structureStore';
import { colorValue, pathTo, type SubnetIndex } from '@/modules/structure/tree';
import type { CommandError } from '@/types/generated/CommandError';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import {
  explorerModel,
  explorerRows,
  GROUPS_AFTER,
  GROUPS_BEFORE,
  homeRowKey,
  RECENT_LIMIT,
  type ExplorerGroup,
  type Row,
} from './explorerRows';
import { useExplorer } from './explorerStore';
import { useWorkspace } from './workspaceStore';

export { RECENT_LIMIT };
export const EXPLORER_LIST_ID = 'pw-explorer-list';
const ROW_HEIGHT = 26;
const OVERSCAN = 8;
const MAX_TITLE = 300;
/** Drag data of Explorer rows (not files from outside). */
export const EXPLORER_DRAG_TYPE = 'application/x-project-write-item';

type DocumentRow = Extract<Row, { kind: 'document' }>;
type DraggedItem = { kind: 'document' | 'subnet'; id: string };

/** The row being dragged inside the Explorer, until it is dropped or the drag ends. */
const drag: { item: DraggedItem | null } = { item: null };

function rememberDrag(item: DraggedItem): void {
  drag.item = item;
}

/** The dragged row, once; afterwards nothing is being dragged. */
function takeDrag(): DraggedItem | null {
  const item = drag.item;
  drag.item = null;
  return item;
}

if (typeof window !== 'undefined') {
  window.addEventListener('dragend', () => {
    drag.item = null;
  });
}
type SubnetRow = Extract<Row, { kind: 'subnet' }>;

function FormatIcon({ format }: { format: DocumentFormat }) {
  if (format === 'md') {
    return <Hash aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
  }
  if (format === 'pwrite') {
    return <PenLine aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
  }
  return <FileText aria-hidden="true" className="size-3.5 shrink-0 text-muted" />;
}

/** A subnet's colour: a dot with a rim, beside its name (never alone). */
export function ColorDot({ color }: { color: string }) {
  return (
    <span
      aria-hidden="true"
      data-color={color}
      className="inline-block size-2.5 shrink-0 rounded-full border border-fg/50"
      style={{ backgroundColor: colorValue(color) }}
    />
  );
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
  const activeIsLibrary = useWorkspace((state) =>
    state.active ? !state.open[state.active]?.external : false,
  );
  const renaming = useWorkspace((state) => state.renaming);
  const openDocument = useWorkspace((state) => state.openDocument);
  const setRenaming = useWorkspace((state) => state.setRenaming);
  const rename = useWorkspace((state) => state.rename);
  const copyDocument = useWorkspace((state) => state.copyDocument);
  const setFavorite = useWorkspace((state) => state.setFavorite);
  const recentFiles = useWorkspace((state) => state.recentFiles);
  const openRecentFile = useWorkspace((state) => state.openRecentFile);
  const forgetRecentFile = useWorkspace((state) => state.forgetRecentFile);
  const trash = useWorkspace((state) => state.trash);
  const deleteDocument = useWorkspace((state) => state.deleteDocument);
  const restoreDocument = useWorkspace((state) => state.restoreDocument);
  const archiveDocument = useWorkspace((state) => state.archiveDocument);
  const purgeDocument = useWorkspace((state) => state.purgeDocument);
  const emptyTrash = useWorkspace((state) => state.emptyTrash);
  const createDocument = useWorkspace((state) => state.createDocument);

  const subnets = useStructure((state) => state.subnets);
  const placements = useStructure((state) => state.byDocument);
  const tags = useStructure((state) => state.tags);
  const structure = useStructure.getState;
  const dialogs = useStructureDialogs.getState;

  const collapsed = useExplorer((state) => state.collapsed);
  const expanded = useExplorer((state) => state.expanded);
  const filter = useExplorer((state) => state.filter);
  const filterVisible = useExplorer((state) => state.filterVisible);
  const reveal = useExplorer((state) => state.reveal);
  const explorer = useExplorer.getState;

  const model = useMemo(
    () => explorerModel({ documents, trash, recentFiles, subnets, placements, filter, locale }),
    [documents, trash, recentFiles, subnets, placements, filter, locale],
  );
  const rows = useMemo(
    () => explorerRows(model, collapsed, expanded, placements),
    [model, collapsed, expanded, placements],
  );
  const roots = useMemo(
    () => (model.index.children.get(null) ?? []).filter((subnet) => model.shown.has(subnet.id)).length,
    [model],
  );
  const topLevel = GROUPS_BEFORE.length + roots + GROUPS_AFTER.length;

  const [selected, setSelected] = useState<string | null>(null);
  const [menu, setMenu] = useState<ContextMenuState | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState({ top: 0, height: 400 });

  // Follow the active tab: its primary place opens and is selected.
  const [followed, setFollowed] = useState<string | null>(null);
  if (active !== followed && activeIsLibrary && active) {
    setFollowed(active);
    setSelected(homeRowKey(active, placements, model.index));
  }
  const index = model.index;
  useEffect(() => {
    if (active && activeIsLibrary) {
      openPlaceOf(active, index);
    }
    // Only when another document becomes active, not on every change of the tree.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, activeIsLibrary]);

  // Shown from elsewhere (breadcrumbs, palette): select and focus the row.
  const [revealed, setRevealed] = useState(reveal);
  if (reveal !== revealed) {
    setRevealed(reveal);
    if (reveal) {
      setSelected(reveal.key);
    }
  }
  useEffect(() => {
    if (reveal) {
      list.current?.focus();
    }
  }, [reveal]);

  // Renaming (F2, menu, palette) happens in the selected row of the document.
  const [renamed, setRenamed] = useState(renaming);
  if (renaming !== renamed) {
    setRenamed(renaming);
    const shown = rows.find((row) => row.key === selected);
    if (renaming && !(shown?.kind === 'document' && shown.document.id === renaming)) {
      setSelected(homeRowKey(renaming, placements, model.index));
    }
  }
  useEffect(() => {
    if (renaming) {
      openPlaceOf(renaming, index);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [renaming]);

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

  const groupLabel = (group: ExplorerGroup) => t(`explorer.group.${group}`);
  const dateTime = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' });
  const subnetName = (id: string | null) => (id ? (model.index.byId.get(id)?.name ?? '') : '');

  const newDocumentIn = async (subnetId: string) => {
    const title = t('document.untitled');
    const id = await createDocument(title);
    if (id) {
      report(structure().assign(id, subnetId));
    }
  };

  const openGroupMenu = (row: Extract<Row, { kind: 'group' }>, position: { x: number; y: number }) => {
    if (row.group !== 'trash' || row.count === 0) {
      return;
    }
    setSelected(row.key);
    setMenu({
      ...position,
      label: groupLabel(row.group),
      items: [{ id: 'emptyTrash', label: t('explorer.emptyTrash'), onSelect: () => void emptyTrash() }],
    });
  };

  const openSubnetMenu = (row: SubnetRow, position: { x: number; y: number }) => {
    setSelected(row.key);
    const { subnet } = row;
    setMenu({
      ...position,
      label: subnet.name,
      items: [
        {
          id: 'newDocument',
          label: t('subnet.newDocument'),
          onSelect: () => void newDocumentIn(subnet.id),
        },
        {
          id: 'newChild',
          label: t('subnet.newChild'),
          onSelect: () => {
            dialogs().openSubnet({ mode: 'create', parentId: subnet.id });
          },
        },
        {
          id: 'edit',
          label: t('subnet.edit'),
          shortcut: 'F2',
          onSelect: () => {
            dialogs().openSubnet({ mode: 'edit', id: subnet.id });
          },
        },
        {
          id: 'move',
          label: t('subnet.moveTo'),
          onSelect: () => {
            dialogs().openPicker({ mode: 'moveSubnet', subnetId: subnet.id });
          },
        },
        {
          id: 'delete',
          label: t('subnet.deleteItem'),
          shortcut: t('keys.delete'),
          onSelect: () => {
            report(structure().deleteSubnet(subnet.id));
          },
        },
      ],
    });
  };

  const openMenu = (row: Extract<Row, { kind: 'document' | 'file' }>, position: { x: number; y: number }) => {
    setSelected(row.key);
    if (row.kind === 'document' && row.group === 'trash') {
      const { document } = row;
      setMenu({
        ...position,
        label: document.title,
        items: [
          { id: 'restore', label: t('explorer.restore'), onSelect: () => void restoreDocument(document.id) },
          { id: 'purge', label: t('explorer.purge'), onSelect: () => void purgeDocument(document.id) },
        ],
      });
      return;
    }
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
    const items: ContextMenuItem[] = [
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
    ];
    if (document.archivedAt === null) {
      items.push(
        {
          id: 'moveTo',
          label: t('structure.moveTo'),
          onSelect: () => {
            dialogs().openPicker({ mode: 'moveDocument', documentId: document.id });
          },
        },
        {
          id: 'assignTo',
          label: t('structure.assignTo'),
          onSelect: () => {
            dialogs().openPicker({ mode: 'assignDocument', documentId: document.id });
          },
        },
      );
    }
    if (row.subnetId) {
      const subnetId = row.subnetId;
      if (row.secondary) {
        items.push({
          id: 'primary',
          label: t('structure.makePrimary', { subnet: subnetName(subnetId) }),
          onSelect: () => {
            report(structure().setPrimary(document.id, subnetId));
          },
        });
      }
      items.push({
        id: 'unassign',
        label: t('structure.removeFrom', { subnet: subnetName(subnetId) }),
        onSelect: () => {
          report(structure().unassign(document.id, subnetId));
        },
      });
    }
    items.push(
      {
        id: 'tags',
        label: t('tags.edit'),
        onSelect: () => {
          dialogs().openTags(document.id);
        },
      },
      {
        id: 'archive',
        label: document.archivedAt === null ? t('explorer.archive') : t('explorer.unarchive'),
        onSelect: () => void archiveDocument(document.id, document.archivedAt === null),
      },
      {
        id: 'delete',
        label: t('explorer.delete'),
        shortcut: t('keys.delete'),
        onSelect: () => void deleteDocument(document.id),
      },
    );
    setMenu({ ...position, label: document.title, items });
  };

  const parentKey = (index: number): string | null => {
    const row = rows[index];
    if (!row) {
      return null;
    }
    if (row.kind === 'document' || row.kind === 'file') {
      if (row.kind === 'document' && row.subnetId) {
        return `s:${row.subnetId}`;
      }
      return row.group;
    }
    if (row.kind === 'subnet' && row.subnet.parentId) {
      return `s:${row.subnet.parentId}`;
    }
    return null;
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (renaming) {
      return;
    }
    const page = Math.max(1, Math.floor(scroll.height / ROW_HEIGHT) - 1);
    const index = selectedIndex < 0 ? -1 : selectedIndex;
    const control = event.ctrlKey || event.metaKey;
    if (control && !event.altKey && (event.key === 'z' || event.key === 'Z' || event.key === 'y')) {
      // Structural undo, apart from the editor's (section 8).
      const redo = event.key === 'y' || event.shiftKey;
      void (redo ? structure().redo() : structure().undo());
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (isContextMenuKey(event)) {
      const element = current ? window.document.getElementById(rowId(current.key)) : null;
      const position = element ? positionBelow(element) : { x: 40, y: 80 };
      if (current?.kind === 'document' || current?.kind === 'file') {
        openMenu(current, position);
      } else if (current?.kind === 'subnet') {
        openSubnetMenu(current, position);
      } else if (current?.kind === 'group') {
        openGroupMenu(current, position);
      }
      event.preventDefault();
      event.stopPropagation();
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
            explorer().toggleGroup(current.group, true);
          }
        } else if (current?.kind === 'subnet' && current.hasContent) {
          if (current.expanded) {
            select(index + 1);
          } else {
            explorer().toggleSubnet(current.subnet.id, true);
          }
        }
        break;
      case 'ArrowLeft':
        if (current?.kind === 'group' && current.expanded) {
          explorer().toggleGroup(current.group, false);
        } else if (current?.kind === 'subnet' && current.expanded) {
          explorer().toggleSubnet(current.subnet.id, false);
        } else {
          const parent = parentKey(index);
          if (parent) {
            setSelected(parent);
          }
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
        if (current?.kind === 'document' && current.group === 'trash') {
          // Documents in the trash do not open: their menu says what can be done.
          const element = window.document.getElementById(rowId(current.key));
          openMenu(current, element ? positionBelow(element) : { x: 40, y: 80 });
        } else if (current?.kind === 'document') {
          void openDocument(current.document.id);
        } else if (current?.kind === 'file') {
          void openRecentFile(current.file.id, current.file.name);
        } else if (current?.kind === 'group' && current.count > 0) {
          explorer().toggleGroup(current.group);
        } else if (current?.kind === 'subnet' && current.hasContent) {
          explorer().toggleSubnet(current.subnet.id);
        }
        break;
      case 'F2':
        if (current?.kind === 'document' && current.group !== 'trash') {
          setRenaming(current.document.id);
        } else if (current?.kind === 'subnet') {
          dialogs().openSubnet({ mode: 'edit', id: current.subnet.id });
        }
        break;
      case 'Delete':
        if (current?.kind === 'document' && current.group === 'trash') {
          void purgeDocument(current.document.id);
        } else if (current?.kind === 'document') {
          void deleteDocument(current.document.id);
        } else if (current?.kind === 'subnet') {
          report(structure().deleteSubnet(current.subnet.id));
        }
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  };

  // --- Drag & drop inside the Explorer ---------------------------------------

  const startDrag = (event: DragEvent<HTMLDivElement>, item: DraggedItem) => {
    // WebKit does not always hand custom types on: the item is kept here too.
    rememberDrag(item);
    try {
      event.dataTransfer.setData(EXPLORER_DRAG_TYPE, JSON.stringify(item));
      event.dataTransfer.effectAllowed = 'copyMove';
    } catch {
      // The kept item is enough.
    }
  };
  const carriesItem = (event: DragEvent<HTMLDivElement>) =>
    event.dataTransfer.types.includes(EXPLORER_DRAG_TYPE) ||
    (drag.item !== null && !event.dataTransfer.types.includes('Files'));
  const draggedItem = (event: DragEvent<HTMLDivElement>): { kind?: unknown; id?: unknown } | null => {
    const kept = takeDrag();
    try {
      const data = event.dataTransfer.getData(EXPLORER_DRAG_TYPE);
      if (data !== '') {
        return JSON.parse(data) as { kind?: unknown; id?: unknown };
      }
    } catch {
      // Falls back to the kept item.
    }
    return kept;
  };
  const dropHandlers = (key: string, target: { subnetId: string | null }) => ({
    onDragOver: (event: DragEvent<HTMLDivElement>) => {
      if (!carriesItem(event)) {
        return;
      }
      event.preventDefault();
      event.dataTransfer.dropEffect = event.ctrlKey || event.metaKey ? 'copy' : 'move';
      setDropTarget(key);
    },
    onDragLeave: () => {
      setDropTarget((previous) => (previous === key ? null : previous));
    },
    onDrop: (event: DragEvent<HTMLDivElement>) => {
      if (!carriesItem(event)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      setDropTarget(null);
      const item = draggedItem(event);
      if (!item || typeof item.id !== 'string') {
        return;
      }
      const id = item.id;
      if (item.kind === 'document') {
        if (target.subnetId && (event.ctrlKey || event.metaKey)) {
          report(structure().assign(id, target.subnetId));
        } else {
          report(structure().moveDocument(id, target.subnetId));
        }
      } else if (item.kind === 'subnet' && target.subnetId && target.subnetId !== id) {
        report(structure().moveSubnet(id, target.subnetId));
      }
    },
  });

  const tagName = (id: string) => tags.find((tag) => tag.id === id)?.name ?? '';
  const [tagQuery, setTagQuery] = useState('');
  const [tagListOpen, setTagListOpen] = useState(false);
  const tagChoices = useMemo(() => {
    const query = tagQuery.trim().toLocaleLowerCase(locale);
    return tags
      .filter((tag) => !filter.tagIds.includes(tag.id))
      .filter((tag) => query === '' || tag.name.toLocaleLowerCase(locale).includes(query))
      .slice(0, 50);
  }, [tags, tagQuery, filter.tagIds, locale]);

  return (
    <section aria-labelledby="explorer-title" className="flex h-full min-h-0 flex-col" data-testid="explorer">
      <header className="flex h-9 shrink-0 items-center justify-between px-3">
        <h2 id="explorer-title" className="text-ui font-semibold tracking-wide text-muted uppercase">
          {t('explorer.title')}
        </h2>
        <div className="flex items-center">
          <button
            type="button"
            className={`inline-flex size-7 items-center justify-center rounded-pw hover:bg-surface ${
              filterVisible ? 'text-accent' : 'text-fg'
            }`}
            aria-label={t('explorer.filter')}
            aria-pressed={filterVisible}
            title={t('explorer.filter')}
            data-testid="explorer-filter-toggle"
            onClick={() => {
              explorer().showFilter(!filterVisible);
            }}
          >
            <Filter aria-hidden="true" className="size-4" />
          </button>
          <button
            type="button"
            className="inline-flex size-7 items-center justify-center rounded-pw text-fg hover:bg-surface"
            aria-label={t('command.subnet.new')}
            title={t('command.subnet.new')}
            data-testid="explorer-new-subnet"
            onClick={() => {
              dialogs().openSubnet({ mode: 'create', parentId: null });
            }}
          >
            <Network aria-hidden="true" className="size-4" />
          </button>
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
        </div>
      </header>
      {filterVisible ? (
        <div className="text-ui flex shrink-0 flex-col gap-1 px-3 pb-2" data-testid="explorer-filter">
          <input
            type="search"
            className="text-ui h-7 rounded-pw border border-muted bg-surface px-2 text-fg"
            placeholder={t('explorer.filterText')}
            aria-label={t('explorer.filterText')}
            data-testid="explorer-filter-text"
            value={filter.text}
            onChange={(event) => {
              explorer().setFilterText(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape' && filter.text !== '') {
                event.stopPropagation();
                explorer().setFilterText('');
              }
            }}
          />
          <div className="flex flex-wrap items-center gap-1">
            {filter.tagIds.map((id) => (
              <button
                key={id}
                type="button"
                data-filter-tag={id}
                className="inline-flex h-6 items-center gap-1 rounded-pw border border-accent px-1.5 text-fg"
                aria-label={t('explorer.filterTagRemove', { tag: tagName(id) })}
                onClick={() => {
                  explorer().toggleFilterTag(id);
                }}
              >
                # {tagName(id)}
                <X aria-hidden="true" className="size-3" />
              </button>
            ))}
            <button
              type="button"
              className="inline-flex h-6 items-center rounded-pw border border-muted px-1.5 text-fg disabled:opacity-50"
              aria-expanded={tagListOpen}
              data-testid="explorer-filter-tags"
              disabled={tags.length === 0}
              onClick={() => {
                setTagListOpen(!tagListOpen);
              }}
            >
              {t('explorer.filterTags')}
            </button>
            {filter.text !== '' || filter.tagIds.length > 0 ? (
              <button
                type="button"
                className="inline-flex h-6 items-center rounded-pw px-1.5 text-muted hover:text-fg"
                data-testid="explorer-filter-clear"
                onClick={() => {
                  explorer().clearFilter();
                }}
              >
                {t('explorer.filterClear')}
              </button>
            ) : null}
          </div>
          {tagListOpen ? (
            <div
              className="flex flex-col gap-1 rounded-pw border border-muted p-1"
              data-testid="explorer-filter-tag-list"
            >
              <input
                type="search"
                className="text-ui h-6 rounded-pw border border-muted bg-surface px-1.5 text-fg"
                aria-label={t('explorer.filterTagSearch')}
                placeholder={t('explorer.filterTagSearch')}
                value={tagQuery}
                onChange={(event) => {
                  setTagQuery(event.target.value);
                }}
              />
              <ul className="max-h-40 overflow-y-auto">
                {tagChoices.map((tag) => (
                  <li key={tag.id}>
                    <button
                      type="button"
                      data-tag-choice={tag.id}
                      className="w-full truncate rounded-pw px-1.5 py-0.5 text-left text-fg hover:bg-surface"
                      onClick={() => {
                        explorer().toggleFilterTag(tag.id);
                      }}
                    >
                      # {tag.name}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {model.filtering ? (
            <p role="status" className="text-muted">
              {t('explorer.filterActive')}
            </p>
          ) : null}
        </div>
      ) : null}
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
            } ${dropTarget === row.key ? 'outline-2 outline-offset-[-2px] outline-accent outline-dashed' : ''}`;
            if (row.kind === 'group') {
              const Chevron = row.expanded ? ChevronDown : ChevronRight;
              const position =
                row.group === 'favorites' || row.group === 'recent'
                  ? GROUPS_BEFORE.indexOf(row.group) + 1
                  : GROUPS_BEFORE.length + roots + GROUPS_AFTER.indexOf(row.group) + 1;
              return (
                <div
                  key={row.key}
                  id={rowId(row.key)}
                  role="treeitem"
                  aria-level={1}
                  aria-setsize={topLevel}
                  aria-posinset={position}
                  aria-expanded={row.count > 0 ? row.expanded : undefined}
                  aria-selected={isSelected}
                  data-group={row.group}
                  className={`${base} px-1.5 font-semibold`}
                  style={style}
                  onClick={() => {
                    setSelected(row.key);
                    if (row.count > 0) {
                      explorer().toggleGroup(row.group);
                    }
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    openGroupMenu(row, { x: event.clientX, y: event.clientY });
                  }}
                  {...(row.group === 'unassigned' ? dropHandlers(row.key, { subnetId: null }) : {})}
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
            if (row.kind === 'subnet') {
              const Chevron = row.expanded ? ChevronDown : ChevronRight;
              const { subnet } = row;
              return (
                <div
                  key={row.key}
                  id={rowId(row.key)}
                  role="treeitem"
                  aria-level={row.level}
                  aria-setsize={row.level === 1 ? topLevel : row.size}
                  aria-posinset={row.level === 1 ? GROUPS_BEFORE.length + row.position : row.position}
                  aria-expanded={row.hasContent ? row.expanded : undefined}
                  aria-selected={isSelected}
                  aria-label={
                    subnet.description
                      ? `${subnet.name}, ${t('subnet.label')}: ${subnet.description}`
                      : `${subnet.name}, ${t('subnet.label')}`
                  }
                  data-subnet={subnet.id}
                  title={subnet.description || undefined}
                  draggable
                  className={`${base} pr-3`}
                  style={{ ...style, paddingLeft: 6 + (row.level - 1) * 14 }}
                  onClick={() => {
                    setSelected(row.key);
                    if (row.hasContent) {
                      explorer().toggleSubnet(subnet.id);
                    }
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    openSubnetMenu(row, { x: event.clientX, y: event.clientY });
                  }}
                  onDragStart={(event) => {
                    startDrag(event, { kind: 'subnet', id: subnet.id });
                  }}
                  {...dropHandlers(row.key, { subnetId: subnet.id })}
                >
                  <Chevron
                    aria-hidden="true"
                    className={`size-3.5 shrink-0 text-muted ${row.hasContent ? '' : 'invisible'}`}
                  />
                  <ColorDot color={subnet.color} />
                  <span className="truncate">{subnet.name}</span>
                  {row.count > 0 ? <span className="ml-auto text-muted">{row.count}</span> : null}
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
            return (
              <DocumentRowView
                key={row.key}
                row={row}
                isSelected={isSelected}
                base={base}
                style={style}
                active={row.document.id === active}
                renaming={renaming === row.document.id && isSelected}
                dateTime={dateTime}
                subnetName={subnetName}
                onSelect={() => {
                  setSelected(row.key);
                  if (row.group !== 'trash') {
                    void openDocument(row.document.id);
                  }
                }}
                onMenu={(position) => {
                  openMenu(row, position);
                }}
                onRenamed={(title) => {
                  if (title === null || title === row.document.title) {
                    setRenaming(null);
                  } else {
                    void rename(row.document.id, title);
                  }
                  list.current?.focus();
                }}
                onDragStart={(event) => {
                  startDrag(event, { kind: 'document', id: row.document.id });
                }}
              />
            );
          })}
        </div>
        {listLoaded && documents.length === 0 && subnets.length === 0 ? (
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

interface DocumentRowViewProps {
  row: DocumentRow;
  isSelected: boolean;
  base: string;
  style: { top: number; height: number };
  active: boolean;
  renaming: boolean;
  dateTime: Intl.DateTimeFormat;
  subnetName: (id: string | null) => string;
  onSelect: () => void;
  onMenu: (position: { x: number; y: number }) => void;
  onRenamed: (title: string | null) => void;
  onDragStart: (event: DragEvent<HTMLDivElement>) => void;
}

function DocumentRowView({
  row,
  isSelected,
  base,
  style,
  active,
  renaming,
  dateTime,
  subnetName,
  onSelect,
  onMenu,
  onRenamed,
  onDragStart,
}: DocumentRowViewProps) {
  const t = useT();
  const { document } = row;
  const inTrash = row.group === 'trash';
  return (
    <div
      id={rowId(row.key)}
      role="treeitem"
      aria-level={row.level}
      aria-setsize={row.size}
      aria-posinset={row.position}
      aria-selected={isSelected}
      data-document={document.id}
      data-group-member={row.group}
      data-subnet-member={row.subnetId ?? undefined}
      data-secondary={row.secondary ? true : undefined}
      draggable={!inTrash && !renaming}
      title={
        inTrash && document.deletedAt
          ? t('explorer.deletedAt', { date: dateTime.format(new Date(document.deletedAt)) })
          : (document.originalName ?? undefined)
      }
      className={`${base} pr-3 ${active ? 'font-semibold' : ''} ${inTrash ? 'text-muted' : ''}`}
      style={{ ...style, paddingLeft: 6 + (row.level - 1) * 14 + 8 }}
      onClick={onSelect}
      onContextMenu={(event) => {
        event.preventDefault();
        onMenu({ x: event.clientX, y: event.clientY });
      }}
      onDragStart={onDragStart}
    >
      <FormatIcon format={document.format} />
      {renaming ? (
        <RenameInput initial={document.title} label={t('explorer.renameLabel')} onDone={onRenamed} />
      ) : (
        <span className="truncate">{document.title}</span>
      )}
      {row.secondary ? (
        <Link2
          role="img"
          aria-label={t('explorer.secondary', { subnet: subnetName(row.subnetId) })}
          className="size-3 shrink-0 text-muted"
        />
      ) : null}
      {document.favoritedAt !== null && row.group !== 'favorites' ? (
        <Star
          role="img"
          aria-label={t('explorer.isFavorite')}
          className="ml-auto size-3 shrink-0 text-muted"
        />
      ) : null}
    </div>
  );
}

/** Shows what went wrong in a structural action started from the Explorer. */
function report(result: Promise<CommandError | null>): void {
  void result.then((error) => {
    if (error) {
      useUi.getState().showNotice({ tone: 'error', key: 'structure.failed', error });
    }
  });
}

/** Opens the entries on the way to a document's primary place. */
function openPlaceOf(id: string, index: SubnetIndex): void {
  const primary = useStructure.getState().byDocument[id]?.subnetIds.find((subnet) => index.byId.has(subnet));
  if (primary) {
    useExplorer.getState().expandPath(pathTo(index, primary).map((subnet) => subnet.id));
  } else {
    useExplorer.getState().toggleGroup('unassigned', true);
  }
}

function rowId(key: string): string {
  return `pw-explorer-${key.replaceAll(':', '-')}`;
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
