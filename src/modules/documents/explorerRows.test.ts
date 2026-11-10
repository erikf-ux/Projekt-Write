import { describe, expect, it } from 'vitest';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { SubnetSummary } from '@/types/generated/SubnetSummary';
import type { Placement } from '@/modules/structure/structureStore';
import {
  explorerModel,
  explorerRows,
  homeRowKey,
  NO_FILTER,
  type ExplorerFilter,
  type Row,
} from './explorerRows';

function doc(id: string, title: string, extra: Partial<DocumentSummary> = {}): DocumentSummary {
  return {
    id,
    title,
    format: 'md',
    sizeBytes: 0,
    createdAt: '2026-11-05T10:00:00.000Z',
    updatedAt: '2026-11-05T10:00:00.000Z',
    originalName: null,
    favoritedAt: null,
    lastOpenedAt: null,
    archivedAt: null,
    deletedAt: null,
    ...extra,
  };
}

function subnet(id: string, name: string, parentId: string | null = null): SubnetSummary {
  return {
    id,
    name,
    description: '',
    color: 'blue',
    parentId,
    createdAt: '2026-11-05T10:00:00.000Z',
    updatedAt: '2026-11-05T10:00:00.000Z',
  };
}

const subnets = [subnet('p', 'Projekte'), subnet('r', 'Roman', 'p'), subnet('w', 'Wissen')];
const documents = [doc('k1', 'Kapitel 1'), doc('k2', 'Kapitel 2'), doc('q', 'Quellen'), doc('n', 'Notiz')];
const placements: Record<string, Placement> = {
  k1: { subnetIds: ['r'], tagIds: ['entwurf'] },
  // Primary in Wissen, also in Roman.
  q: { subnetIds: ['w', 'r'], tagIds: ['quelle', 'entwurf'] },
  k2: { subnetIds: ['r'], tagIds: [] },
};

function rows(filter: ExplorerFilter = NO_FILTER, expanded: string[] = ['p', 'r', 'w']): Row[] {
  const model = explorerModel({
    documents,
    trash: [],
    recentFiles: [],
    subnets,
    placements,
    filter,
    locale: 'de',
  });
  return explorerRows(model, new Set(['archive', 'trash']), new Set(expanded), placements);
}

function outline(list: Row[]): string[] {
  return list.map((row) => {
    switch (row.kind) {
      case 'group':
        return `${row.group} (${String(row.count)})`;
      case 'subnet':
        return `${'  '.repeat(row.level - 1)}[${row.subnet.name}]`;
      case 'document':
        return `${'  '.repeat(row.level - 1)}${row.document.title}${row.secondary ? ' +' : ''}`;
      case 'media':
        return `${'  '.repeat(row.level - 1)}<${row.media.title}>${row.secondary ? ' +' : ''}`;
      case 'file':
        return row.file.name;
    }
  });
}

describe('Explorer rows', () => {
  it('builds the subnet tree between the entries; a document in two subnets appears in both', () => {
    expect(outline(rows())).toEqual([
      'favorites (0)',
      'recent (0)',
      '[Projekte]',
      '  [Roman]',
      '    Kapitel 1',
      '    Kapitel 2',
      '    Quellen +',
      '[Wissen]',
      '  Quellen',
      'unassigned (1)',
      '  Notiz',
      'media (0)',
      'archive (0)',
      'trash (0)',
    ]);
    // Both rows are the same element.
    const quellen = rows().filter((row) => row.kind === 'document' && row.document.id === 'q');
    expect(quellen).toHaveLength(2);
    expect(new Set(quellen.map((row) => row.kind === 'document' && row.document))).toHaveProperty('size', 1);
    expect(
      homeRowKey(
        'q',
        placements,
        explorerModel({
          documents,
          trash: [],
          recentFiles: [],
          subnets,
          placements,
          filter: NO_FILTER,
          locale: 'de',
        }).index,
      ),
    ).toBe('s:w:q');
  });

  it('shows only the rows of open subnets', () => {
    expect(outline(rows(NO_FILTER, []))).toEqual([
      'favorites (0)',
      'recent (0)',
      '[Projekte]',
      '[Wissen]',
      'unassigned (1)',
      '  Notiz',
      'media (0)',
      'archive (0)',
      'trash (0)',
    ]);
  });

  it('filters by every chosen tag and by title, keeping the subnets that hold a match', () => {
    expect(outline(rows({ text: '', tagIds: ['entwurf', 'quelle'] }, []))).toEqual([
      'favorites (0)',
      'recent (0)',
      '[Projekte]',
      '  [Roman]',
      '    Quellen +',
      '[Wissen]',
      '  Quellen',
      'unassigned (0)',
      'media (0)',
      'archive (0)',
      'trash (0)',
    ]);
    expect(outline(rows({ text: 'notiz', tagIds: [] }))).toEqual([
      'favorites (0)',
      'recent (0)',
      'unassigned (1)',
      '  Notiz',
      'media (0)',
      'archive (0)',
      'trash (0)',
    ]);
  });

  it('stays fast with the size of the reference library', () => {
    // 10,000 documents, 500 subnets in four levels, 1,000 tags (section 6).
    const many: SubnetSummary[] = [];
    for (let index = 0; index < 500; index += 1) {
      const parent = index < 20 ? null : `s${String(Math.floor((index - 20) / 4))}`;
      many.push(subnet(`s${String(index)}`, `Subnetz ${String(index)}`, parent));
    }
    const docs: DocumentSummary[] = [];
    const where: Record<string, Placement> = {};
    for (let index = 0; index < 10_000; index += 1) {
      const id = `d${String(index)}`;
      docs.push(
        doc(id, `Dokument ${String(index)}`, {
          lastOpenedAt: index % 7 === 0 ? '2026-11-05T10:00:00.000Z' : null,
        }),
      );
      where[id] = {
        subnetIds: index % 7 === 3 ? [] : [`s${String(index % 500)}`, `s${String((index * 13) % 500)}`],
        tagIds: [`t${String(index % 1000)}`, `t${String((index * 7) % 1000)}`],
      };
    }
    const all = new Set(many.map((entry) => entry.id));
    const started = performance.now();
    const model = explorerModel({
      documents: docs,
      trash: [],
      recentFiles: [],
      subnets: many,
      placements: where,
      filter: NO_FILTER,
      locale: 'de',
    });
    const list = explorerRows(model, new Set(), all, where);
    const fullTree = performance.now() - started;
    const filterStart = performance.now();
    const filtered = explorerModel({
      documents: docs,
      trash: [],
      recentFiles: [],
      subnets: many,
      placements: where,
      filter: { text: '', tagIds: ['t7'] },
      locale: 'de',
    });
    explorerRows(filtered, new Set(), new Set(), where);
    const filter = performance.now() - filterStart;
    // Every subnet open: 500 subnet rows, the documents of every membership.
    expect(list.length).toBeGreaterThan(18_000);
    // jsdom on the CI machines is slower than the app; the E2E test measures there.
    expect(fullTree).toBeLessThan(1_500);
    expect(filter).toBeLessThan(1_000);
  });
});
