/**
 * The rows of the Explorer (sections 13 and 16; ADR 0017, decision 11):
 * Favoriten, Zuletzt verwendet, the subnets of the top level as a tree
 * (subnets before documents, both by name), Nicht zugeordnet, Archiv,
 * Papierkorb. A document in several subnets appears in each; it stays one
 * element. The filter (decision 13) keeps documents with every chosen tag
 * and the text in the title, and the subnets that hold any of them.
 *
 * Pure functions: the Explorer renders the visible slice of these rows.
 */
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { RecentFileEntry } from '@/types/generated/RecentFileEntry';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';
import { NO_PLACEMENT, type Placement } from '@/modules/structure/structureStore';
import { buildIndex, type SubnetIndex } from '@/modules/structure/tree';

/** Entries of "Zuletzt verwendet". */
export const RECENT_LIMIT = 10;

export type ExplorerGroup = 'favorites' | 'recent' | 'unassigned' | 'archive' | 'trash';
/** Entries above and below the subnets. */
export const GROUPS_BEFORE: readonly ExplorerGroup[] = ['favorites', 'recent'];
export const GROUPS_AFTER: readonly ExplorerGroup[] = ['unassigned', 'archive', 'trash'];
/** Entries closed until the user opens them. */
export const CLOSED_AT_START: readonly ExplorerGroup[] = ['archive', 'trash'];

export interface ExplorerFilter {
  text: string;
  tagIds: readonly string[];
}

export const NO_FILTER: ExplorerFilter = { text: '', tagIds: [] };

export function isFiltering(filter: ExplorerFilter): boolean {
  return filter.text.trim() !== '' || filter.tagIds.length > 0;
}

/** An entry of "Zuletzt verwendet": a library document or an external file. */
export type RecentEntry =
  { kind: 'document'; document: DocumentSummary } | { kind: 'file'; file: RecentFileEntry };

export type Row =
  | { kind: 'group'; key: string; group: ExplorerGroup; count: number; expanded: boolean }
  | {
      kind: 'subnet';
      key: string;
      subnet: SubnetSummary;
      level: number;
      /** Documents directly in it (with the filter: the matching ones). */
      count: number;
      /** Something to open: subnets or documents below it. */
      hasContent: boolean;
      expanded: boolean;
      position: number;
      size: number;
    }
  | {
      kind: 'document';
      key: string;
      /** The entry it is listed in; `subnet` for a place in the tree. */
      group: ExplorerGroup | 'subnet';
      /** The subnet of this row (tree rows only). */
      subnetId: string | null;
      /** A further, not primary, membership. */
      secondary: boolean;
      document: DocumentSummary;
      level: number;
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

/** Everything the rows are made of; recomputed when data or filter change. */
export interface ExplorerModel {
  groups: Record<Exclude<ExplorerGroup, 'recent'>, DocumentSummary[]>;
  recent: RecentEntry[];
  index: SubnetIndex;
  /** Documents (not archived, matching the filter) per subnet, by title. */
  bySubnet: ReadonlyMap<string, DocumentSummary[]>;
  /** Subnets shown: all, or with the filter those holding a match. */
  shown: ReadonlySet<string>;
  filtering: boolean;
}

interface ModelInput {
  documents: readonly DocumentSummary[];
  trash: readonly DocumentSummary[];
  recentFiles: readonly RecentFileEntry[];
  subnets: readonly SubnetSummary[];
  placements: Readonly<Record<string, Placement>>;
  filter: ExplorerFilter;
  locale: string;
}

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

export function explorerModel(input: ModelInput): ExplorerModel {
  const { locale, filter, placements } = input;
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  const byTitle = (a: DocumentSummary, b: DocumentSummary) =>
    collator.compare(a.title, b.title) || a.id.localeCompare(b.id);
  const text = filter.text.trim().toLocaleLowerCase(locale);
  const filtering = isFiltering(filter);
  const matches = (document: DocumentSummary) => {
    if (text !== '' && !document.title.toLocaleLowerCase(locale).includes(text)) {
      return false;
    }
    if (filter.tagIds.length === 0) {
      return true;
    }
    const tags = (placements[document.id] ?? NO_PLACEMENT).tagIds;
    return filter.tagIds.every((tag) => tags.includes(tag));
  };

  const index = buildIndex(input.subnets, locale);
  const visible = input.documents.filter(matches).sort(byTitle);
  const live = visible.filter((document) => document.archivedAt === null);
  const bySubnet = new Map<string, DocumentSummary[]>();
  const unassigned: DocumentSummary[] = [];
  for (const document of live) {
    const subnetIds = (placements[document.id] ?? NO_PLACEMENT).subnetIds.filter((id) => index.byId.has(id));
    if (subnetIds.length === 0) {
      unassigned.push(document);
      continue;
    }
    for (const id of subnetIds) {
      const list = bySubnet.get(id);
      if (list) {
        list.push(document);
      } else {
        bySubnet.set(id, [document]);
      }
    }
  }

  // With a filter, a subnet shows when it or a subnet below holds a match.
  const shown = new Set<string>();
  const mark = (parent: string | null): boolean => {
    let any = false;
    for (const subnet of index.children.get(parent) ?? []) {
      const below = mark(subnet.id);
      if (!filtering || below || (bySubnet.get(subnet.id)?.length ?? 0) > 0) {
        shown.add(subnet.id);
        any = true;
      }
    }
    return any;
  };
  mark(null);

  const recentDocuments = live
    .filter((document) => document.lastOpenedAt !== null)
    .sort((a, b) => (b.lastOpenedAt ?? '').localeCompare(a.lastOpenedAt ?? '') || a.id.localeCompare(b.id))
    .slice(0, RECENT_LIMIT);
  const files = filtering
    ? filter.tagIds.length > 0
      ? []
      : input.recentFiles.filter((file) => file.name.toLocaleLowerCase(locale).includes(text))
    : input.recentFiles;

  return {
    groups: {
      favorites: live.filter((document) => document.favoritedAt !== null),
      unassigned,
      archive: visible.filter((document) => document.archivedAt !== null),
      trash: input.trash.filter(matches),
    },
    recent: recentEntries(recentDocuments, files),
    index,
    bySubnet,
    shown,
    filtering,
  };
}

/** The rows in order; closed entries and subnets contribute only their own row. */
export function explorerRows(
  model: ExplorerModel,
  collapsed: ReadonlySet<ExplorerGroup>,
  expanded: ReadonlySet<string>,
  placements: Readonly<Record<string, Placement>>,
): Row[] {
  const rows: Row[] = [];
  const group = (name: ExplorerGroup) => {
    const members: RecentEntry[] =
      name === 'recent'
        ? model.recent
        : model.groups[name].map((document) => ({ kind: 'document', document }));
    const open = !collapsed.has(name) && members.length > 0;
    rows.push({ kind: 'group', key: name, group: name, count: members.length, expanded: open });
    if (!open) {
      return;
    }
    members.forEach((entry, index) => {
      const position = index + 1;
      const size = members.length;
      rows.push(
        entry.kind === 'document'
          ? {
              kind: 'document',
              key: `${name}:${entry.document.id}`,
              group: name,
              subnetId: null,
              secondary: false,
              document: entry.document,
              level: 2,
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
  };

  const subnets = (parent: string | null, level: number) => {
    const children = (model.index.children.get(parent) ?? []).filter((subnet) => model.shown.has(subnet.id));
    const documents = parent === null ? [] : (model.bySubnet.get(parent) ?? []);
    const size = children.length + documents.length;
    children.forEach((subnet, index) => {
      const count = model.bySubnet.get(subnet.id)?.length ?? 0;
      const hasContent =
        count > 0 || (model.index.children.get(subnet.id) ?? []).some((child) => model.shown.has(child.id));
      const open = hasContent && (model.filtering || expanded.has(subnet.id));
      rows.push({
        kind: 'subnet',
        key: `s:${subnet.id}`,
        subnet,
        level,
        count,
        hasContent,
        expanded: open,
        position: index + 1,
        size: parent === null ? children.length : size,
      });
      if (open) {
        subnets(subnet.id, level + 1);
      }
    });
    if (parent === null) {
      return;
    }
    documents.forEach((document, index) => {
      rows.push({
        kind: 'document',
        key: `s:${parent}:${document.id}`,
        group: 'subnet',
        subnetId: parent,
        secondary: (placements[document.id] ?? NO_PLACEMENT).subnetIds[0] !== parent,
        document,
        level,
        position: children.length + index + 1,
        size,
      });
    });
  };

  GROUPS_BEFORE.forEach(group);
  subnets(null, 1);
  GROUPS_AFTER.forEach(group);
  return rows;
}

/** Key of the row an open document is shown in: its primary place. */
export function homeRowKey(
  id: string,
  placements: Readonly<Record<string, Placement>>,
  index: SubnetIndex,
): string {
  const primary = (placements[id] ?? NO_PLACEMENT).subnetIds.find((subnet) => index.byId.has(subnet));
  return primary ? `s:${primary}:${id}` : `unassigned:${id}`;
}
