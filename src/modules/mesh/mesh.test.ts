import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshNode } from '@/types/generated/MeshNode';
import { CLUSTER_SIZE, ELEMENT_SIZE, HULL_HEADER, place } from './layout';
import { nearest } from './MeshPanel';
import { useMesh } from './meshStore';

function node(id: string, extra: Partial<MeshNode> = {}): MeshNode {
  return {
    id,
    kind: 'document',
    mediaKind: null,
    title: id,
    color: null,
    archived: false,
    parentId: null,
    open: false,
    count: 0,
    childSubnets: 0,
    x: null,
    y: null,
    pinned: false,
    ...extra,
  };
}

function graph(nodes: MeshNode[]): MeshGraph {
  return { view: 'global', scopeId: '', nodes, edges: [], hidden: 0, breadcrumb: [] };
}

describe('placement', () => {
  it('sizes an open hull to hold its children below its title', () => {
    const placed = place(
      graph([
        node('a', { kind: 'subnet', open: true }),
        node('b', { kind: 'subnet' }),
        ...Array.from({ length: 9 }, (_, i) => node(`d${String(i)}`, { parentId: 'a' })),
      ]),
    );
    const hull = placed.get('a');
    expect(hull?.parentId).toBeNull();
    for (let i = 0; i < 9; i += 1) {
      const child = placed.get(`d${String(i)}`);
      expect(child?.parentId).toBe('a');
      expect(child?.y ?? 0).toBeGreaterThanOrEqual(HULL_HEADER);
      expect((child?.x ?? 0) + ELEMENT_SIZE.width).toBeLessThanOrEqual(hull?.width ?? 0);
      expect((child?.y ?? 0) + ELEMENT_SIZE.height).toBeLessThanOrEqual(hull?.height ?? 0);
    }
    expect(placed.get('b')?.width).toBe(CLUSTER_SIZE.width);
  });

  it('keeps stored positions and grows the hull around a moved child', () => {
    const placed = place(
      graph([
        node('a', { kind: 'subnet', open: true, x: 500, y: 40 }),
        node('d', { parentId: 'a', x: 900, y: 600 }),
      ]),
    );
    expect(placed.get('a')).toMatchObject({ x: 500, y: 40 });
    expect(placed.get('d')).toMatchObject({ x: 900, y: 600 });
    expect(placed.get('a')?.width).toBeGreaterThanOrEqual(900 + ELEMENT_SIZE.width);
  });
});

describe('keyboard', () => {
  const nodes = [
    { id: 'left', x: 0, y: 100 },
    { id: 'right', x: 300, y: 110 },
    { id: 'far-right-up', x: 280, y: -200 },
    { id: 'down', x: 20, y: 400 },
  ];

  it('moves to the nearest node in the direction of the arrow', () => {
    const from = { x: 100, y: 100 };
    expect(nearest(from, 'ArrowRight', nodes)).toBe('right');
    expect(nearest(from, 'ArrowLeft', nodes)).toBe('left');
    expect(nearest(from, 'ArrowDown', nodes)).toBe('down');
    expect(nearest(from, 'ArrowUp', nodes)).toBe('far-right-up');
    expect(nearest({ x: 1000, y: 0 }, 'ArrowRight', nodes)).toBeNull();
  });
});

describe('store', () => {
  afterEach(() => {
    clearMocks();
  });

  it('removes a connection and puts it back on undo, with its id', async () => {
    const calls: { command: string; request: unknown }[] = [];
    const connection = {
      id: '1f2e3d4c-5b6a-4978-8f9e-0d1c2b3a4f5e',
      nodeA: '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11',
      nodeB: '5d2c1e0a-8a1b-4c2d-9e3f-4a5b6c7d8e9f',
      kind: 'manual',
      note: null,
      createdAt: '2026-11-12T10:00:00.000Z',
    };
    mockIPC((command, args) => {
      calls.push({ command, request: (args as { request: unknown }).request });
      switch (command) {
        case 'mesh_graph':
          return graph([node('a')]);
        case 'mesh_disconnect':
        case 'mesh_restore_connection':
          return connection;
        default:
          return null;
      }
    });
    await useMesh.getState().load();
    expect(useMesh.getState().graph?.nodes).toHaveLength(1);
    expect(await useMesh.getState().disconnect(connection.id)).toBe(true);
    expect(useMesh.getState().canUndo()).toBe(true);
    expect(await useMesh.getState().undo()).toBe(true);
    const restore = calls.find((call) => call.command === 'mesh_restore_connection');
    expect(restore?.request).toEqual({
      id: connection.id,
      nodeA: connection.nodeA,
      nodeB: connection.nodeB,
      kind: 'manual',
    });
    expect(useMesh.getState().canUndo()).toBe(false);
  });

  it('asks for the open clusters of the view it shows', async () => {
    const requests: unknown[] = [];
    mockIPC((command, args) => {
      if (command === 'mesh_graph') {
        requests.push((args as { request: unknown }).request);
        return graph([]);
      }
      return null;
    });
    await useMesh.getState().show({ view: 'global', scopeId: '' });
    await useMesh.getState().toggle('unassigned');
    await useMesh.getState().show({ view: 'focus', scopeId: '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11' });
    await useMesh.getState().back();
    expect(requests).toEqual([
      { view: 'global', expanded: [] },
      { view: 'global', expanded: ['unassigned'] },
      { view: 'focus', scopeId: '0b0e5a3e-0c5f-4a43-9f39-3a3b9a9d2c11', expanded: [], depth: 1 },
      { view: 'global', expanded: ['unassigned'] },
    ]);
  });
});
