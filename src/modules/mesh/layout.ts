/**
 * Where the nodes of a view go when nothing is stored for them (ADR 0022,
 * decision 6): rows of nodes, open hulls sized to hold their content.
 * Stored positions win; a hull grows to keep a moved child inside. The
 * automatic layout (ELK.js, in a web worker) replaces this on command.
 *
 * Positions of a node in a hull are relative to the hull (React Flow's
 * `parentId`).
 */
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshNode } from '@/types/generated/MeshNode';

export const ELEMENT_SIZE = { width: 176, height: 40 };
export const CLUSTER_SIZE = { width: 200, height: 64 };
/** Space inside a hull: around, above for its title, between nodes. */
export const HULL_PADDING = 20;
export const HULL_HEADER = 34;
export const GAP = 16;

export interface Placed {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  parentId: string | null;
}

interface Box {
  width: number;
  height: number;
}

/** Packs boxes into rows about as wide as they are high, in the given order. */
function rows(boxes: readonly Box[]): { x: number; y: number }[] {
  const area = boxes.reduce((sum, box) => sum + (box.width + GAP) * (box.height + GAP), 0);
  const widest = boxes.reduce((most, box) => Math.max(most, box.width), 0);
  const target = Math.max(widest, Math.sqrt(area) * 1.5);
  const spots: { x: number; y: number }[] = [];
  let x = 0;
  let y = 0;
  let rowHeight = 0;
  for (const box of boxes) {
    if (x > 0 && x + box.width > target) {
      x = 0;
      y += rowHeight + GAP;
      rowHeight = 0;
    }
    spots.push({ x, y });
    x += box.width + GAP;
    rowHeight = Math.max(rowHeight, box.height);
  }
  return spots;
}

function own(node: MeshNode): Box {
  if (node.kind === 'document' || node.kind === 'media') {
    return ELEMENT_SIZE;
  }
  return CLUSTER_SIZE;
}

export function place(graph: MeshGraph): Map<string, Placed> {
  const children = new Map<string | null, MeshNode[]>();
  for (const node of graph.nodes) {
    const key = node.parentId ?? null;
    const list = children.get(key);
    if (list) {
      list.push(node);
    } else {
      children.set(key, [node]);
    }
  }
  const placed = new Map<string, Placed>();

  // Sizes from the inside out: a hull fits its children.
  const size = (node: MeshNode): Box => {
    if (!node.open) {
      return own(node);
    }
    const inside = children.get(node.id) ?? [];
    const boxes = inside.map(size);
    const spots = rows(boxes);
    let width = CLUSTER_SIZE.width;
    let height = 0;
    inside.forEach((child, i) => {
      const box = boxes[i] ?? ELEMENT_SIZE;
      const spot = spots[i] ?? { x: 0, y: 0 };
      const x = child.x ?? spot.x + HULL_PADDING;
      const y = child.y ?? spot.y + HULL_HEADER;
      placed.set(child.id, { id: child.id, x, y, width: box.width, height: box.height, parentId: node.id });
      width = Math.max(width, x + box.width + HULL_PADDING);
      height = Math.max(height, y + box.height + HULL_PADDING);
    });
    return { width, height: Math.max(height, HULL_HEADER + HULL_PADDING) };
  };

  const top = children.get(null) ?? [];
  const boxes = top.map(size);
  const spots = rows(boxes);
  top.forEach((node, i) => {
    const box = boxes[i] ?? own(node);
    const spot = spots[i] ?? { x: 0, y: 0 };
    placed.set(node.id, {
      id: node.id,
      x: node.x ?? spot.x,
      y: node.y ?? spot.y,
      width: box.width,
      height: box.height,
      parentId: null,
    });
  });
  return placed;
}
