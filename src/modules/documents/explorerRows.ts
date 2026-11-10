/**
 * The rows of the Explorer (sections 13 and 16; ADR 0017, decision 11;
 * ADR 0019, decision 12): Favoriten, Zuletzt verwendet, the subnets of the
 * top level as a tree (subnets, then documents, then media, each by name),
 * Nicht zugeordnet (documents), Medien (all media), Archiv, Papierkorb. An
 * element in several subnets appears in each; it stays one element. The
 * filter (decision 13) keeps elements with every chosen tag and the text in
 * the title, and the subnets that hold any of them.
 *
 * Pure functions: the Explorer renders the visible slice of these rows.
 */
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import type { RecentFileEntry } from '@/types/generated/RecentFileEntry';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';
import { NO_PLACEMENT, type Placement } from '@/modules/structure/structureStore';
import { buildIndex, type SubnetIndex } from '@/modules/structure/tree';

/** Entries of "Zuletzt verwendet". */
export const RECENT_LIMIT = 10;

export type ExplorerGroup = 'favorites' | 'recent' | 'unassigned' | 'media' | 'archive' | 'trash';
/** Entries above and below the subnets. */
export const GROUPS_BEFORE: readonly ExplorerGroup[] = ['favorites', 'recent'];
export const GROUPS_AFTER: readonly ExplorerGroup[] = ['unassigned', 'media', 'archive', 'trash'];
/** Entries closed until the user opens them. */
export const CLOSED_AT_START: readonly ExplorerGroup[] = ['media', 'archive', 'trash'];

export interface ExplorerFilter {
  text: string;
  tagIds: readonly string[];
}

export const NO_FILTER: ExplorerFilter = { text: '', tagIds: [] };

export function isFiltering(filter: ExplorerFilter): boolean {
  return filter.text.trim() !== '' || filter.tagIds.length > 0;
}

/** An entry of a group: a library document or a medium. */
export type GroupEntry =
  { kind: 'document'; document: DocumentSummary } | { kind: 'media'; media: MediaSummary };

/** An entry of "Zuletzt verwendet": a document, a medium or an external file. */
export type RecentEntry = GroupEntry | { kind: 'file'; file: RecentFileEntry };

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
      kind: 'media';
      key: string;
      /** The entry it is listed in; `subnet` for a place in the tree. */
      group: ExplorerGroup | 'subnet';
      subnetId: string | null;
      secondary: boolean;
      media: MediaSummary;
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
  groups: Record<Exclude<ExplorerGroup, 'recent'>, GroupEntry[]>;
  recent: RecentEntry[];
  index: SubnetIndex;
  /** Documents (not archived, matching the filter) per subnet, by title. */
  bySubnet: ReadonlyMap<string, DocumentSummary[]>;
  /** Media (not archived, matching the filter) per subnet, by title. */
  mediaBySubnet: ReadonlyMap<string, MediaSummary[]>;
  /** Subnets shown: all, or with the filter those holding a match. */
  shown: ReadonlySet<string>;
  filtering: boolean;
}

interface ModelInput {
  documents: readonly DocumentSummary[];
  trash: readonly DocumentSummary[];
  media?: readonly MediaSummary[];
  mediaTrash?: readonly MediaSummary[];
  recentFiles: readonly RecentFileEntry[];
  subnets: readonly SubnetSummary[];
  placements: Readonly<Record<string, Placement>>;
  filter: ExplorerFilter;
  locale: string;
}

/** Library documents, media and external files of "Zuletzt verwendet", newest first. */
export function recentEntries(
  documents: readonly DocumentSummary[],
  files: readonly RecentFileEntry[],
  media: readonly MediaSummary[] = [],
): RecentEntry[] {
  const entries: { at: string; entry: RecentEntry }[] = [
    ...documents.map((document) => ({
      at: document.lastOpenedAt ?? '',
      entry: { kind: 'document', document } as const,
    })),
    ...media.map((medium) => ({
      at: medium.lastOpenedAt ?? '',
      entry: { kind: 'media', media: medium } as const,
    })),
    ...files.map((file) => ({ at: file.openedAt, entry: { kind: 'file', file } as const })),
  ];
  return entries
    .sort((a, b) => b.at.localeCompare(a.at))
    .slice(0, RECENT_LIMIT)
    .map(({ entry }) => entry);
}

/** What does not change with the filter: elements by title and the subnet index. */
export interface ExplorerBase {
  sorted: readonly DocumentSummary[];
  sortedMedia: readonly MediaSummary[];
  index: SubnetIndex;
}

export function explorerBase(
  documents: readonly DocumentSummary[],
  subnets: readonly SubnetSummary[],
  locale: string,
  media: readonly MediaSummary[] = [],
): ExplorerBase {
  const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
  const byTitle = (a: { title: string; id: string }, b: { title: string; id: string }) =>
    collator.compare(a.title, b.title) || a.id.localeCompare(b.id);
  const sorted = [...documents].sort(byTitle);
  const sortedMedia = [...media].sort(byTitle);
  return { sorted, sortedMedia, index: buildIndex(subnets, locale) };
}

export function explorerModel(input: ModelInput, base?: ExplorerBase): ExplorerModel {
  const { locale, filter, placements } = input;
  const { sorted, sortedMedia, index } =
    base ?? explorerBase(input.documents, input.subnets, locale, input.media ?? []);
  const text = filter.text.trim().toLocaleLowerCase(locale);
  const filtering = isFiltering(filter);
  const matches = (element: { id: string; title: string }) => {
    if (text !== '' && !element.title.toLocaleLowerCase(locale).includes(text)) {
      return false;
    }
    if (filter.tagIds.length === 0) {
      return true;
    }
    const tags = (placements[element.id] ?? NO_PLACEMENT).tagIds;
    return filter.tagIds.every((tag) => tags.includes(tag));
  };

  // Sorted once per change of the documents, not per filter.
  const visible = filtering ? sorted.filter(matches) : sorted;
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

  const visibleMedia = filtering ? sortedMedia.filter(matches) : sortedMedia;
  const liveMedia = visibleMedia.filter((medium) => medium.archivedAt === null);
  const mediaBySubnet = new Map<string, MediaSummary[]>();
  for (const medium of liveMedia) {
    for (const id of (placements[medium.id] ?? NO_PLACEMENT).subnetIds) {
      if (!index.byId.has(id)) {
        continue;
      }
      const list = mediaBySubnet.get(id);
      if (list) {
        list.push(medium);
      } else {
        mediaBySubnet.set(id, [medium]);
      }
    }
  }
  const documentEntry = (document: DocumentSummary): GroupEntry => ({ kind: 'document', document });
  const mediaEntry = (medium: MediaSummary): GroupEntry => ({ kind: 'media', media: medium });

  // With a filter, a subnet shows when it or a subnet below holds a match.
  const shown = new Set<string>();
  const mark = (parent: string | null): boolean => {
    let any = false;
    for (const subnet of index.children.get(parent) ?? []) {
      const below = mark(subnet.id);
      if (
        !filtering ||
        below ||
        (bySubnet.get(subnet.id)?.length ?? 0) > 0 ||
        (mediaBySubnet.get(subnet.id)?.length ?? 0) > 0
      ) {
        shown.add(subnet.id);
        any = true;
      }
    }
    return any;
  };
  mark(null);

  const byOpening = (
    a: { lastOpenedAt: string | null; id: string },
    b: { lastOpenedAt: string | null; id: string },
  ) => (b.lastOpenedAt ?? '').localeCompare(a.lastOpenedAt ?? '') || a.id.localeCompare(b.id);
  const recentDocuments = live
    .filter((document) => document.lastOpenedAt !== null)
    .sort(byOpening)
    .slice(0, RECENT_LIMIT);
  const recentMedia = liveMedia
    .filter((medium) => medium.lastOpenedAt !== null)
    .sort(byOpening)
    .slice(0, RECENT_LIMIT);
  const files = filtering
    ? filter.tagIds.length > 0
      ? []
      : input.recentFiles.filter((file) => file.name.toLocaleLowerCase(locale).includes(text))
    : input.recentFiles;

  return {
    groups: {
      favorites: [
        ...live.filter((document) => document.favoritedAt !== null).map(documentEntry),
        ...liveMedia.filter((medium) => medium.favoritedAt !== null).map(mediaEntry),
      ],
      unassigned: unassigned.map(documentEntry),
      media: liveMedia.map(mediaEntry),
      archive: [
        ...visible.filter((document) => document.archivedAt !== null).map(documentEntry),
        ...visibleMedia.filter((medium) => medium.archivedAt !== null).map(mediaEntry),
      ],
      trash: [
        ...input.trash.filter(matches).map(documentEntry),
        ...(input.mediaTrash ?? []).filter(matches).map(mediaEntry),
      ],
    },
    recent: recentEntries(recentDocuments, files, recentMedia),
    index,
    bySubnet,
    mediaBySubnet,
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
    const members: RecentEntry[] = name === 'recent' ? model.recent : model.groups[name];
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
          : entry.kind === 'media'
            ? {
                kind: 'media',
                key: `${name}:m-${entry.media.id}`,
                group: name,
                subnetId: null,
                secondary: false,
                media: entry.media,
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
    const media = parent === null ? [] : (model.mediaBySubnet.get(parent) ?? []);
    const size = children.length + documents.length + media.length;
    children.forEach((subnet, index) => {
      const count =
        (model.bySubnet.get(subnet.id)?.length ?? 0) + (model.mediaBySubnet.get(subnet.id)?.length ?? 0);
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
    media.forEach((medium, index) => {
      rows.push({
        kind: 'media',
        key: `s:${parent}:m-${medium.id}`,
        group: 'subnet',
        subnetId: parent,
        secondary: (placements[medium.id] ?? NO_PLACEMENT).subnetIds[0] !== parent,
        media: medium,
        level,
        position: children.length + documents.length + index + 1,
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
