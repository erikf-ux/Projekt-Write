/**
 * Runs the automatic layout in its worker (elkWorker.ts) and decides what
 * changes: nodes the user moved (pinned) stay where they are, unless the
 * whole view is laid out again on explicit request (section 15).
 */
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshPosition } from '@/types/generated/MeshPosition';
import type { LayoutRequest, LayoutResponse } from './elkWorker';
import { place } from './layout';

let worker: Worker | null = null;
let next = 0;
const waiting = new Map<number, (response: LayoutResponse) => void>();

function layoutWorker(): Worker {
  if (!worker) {
    worker = new Worker(new URL('./elkWorker.ts', import.meta.url), { type: 'module' });
    worker.onmessage = (event: MessageEvent<LayoutResponse>) => {
      const done = waiting.get(event.data.id);
      waiting.delete(event.data.id);
      done?.(event.data);
    };
  }
  return worker;
}

/** Positions ELK gives the nodes of `graph`. */
export async function elkPositions(graph: MeshGraph): Promise<Map<string, { x: number; y: number }>> {
  const sizes = place(graph);
  const request: LayoutRequest = {
    id: ++next,
    nodes: graph.nodes.map((node) => ({
      id: node.id,
      parentId: node.parentId ?? null,
      width: sizes.get(node.id)?.width ?? 176,
      height: sizes.get(node.id)?.height ?? 40,
      open: node.open,
    })),
    edges: graph.edges.map((edge) => ({ id: edge.id, source: edge.source, target: edge.target })),
  };
  const response = await new Promise<LayoutResponse>((resolve) => {
    waiting.set(request.id, resolve);
    layoutWorker().postMessage(request);
  });
  if (response.error) {
    throw new Error(response.error);
  }
  return new Map(response.positions.map((position) => [position.id, { x: position.x, y: position.y }]));
}

/**
 * What to save after a layout: new positions (pinned ones only with
 * `all`), and the positions before, for undo.
 */
export function layoutChanges(
  graph: MeshGraph,
  positions: ReadonlyMap<string, { x: number; y: number }>,
  all: boolean,
): { positions: MeshPosition[]; before: MeshPosition[] } {
  const changed: MeshPosition[] = [];
  const before: MeshPosition[] = [];
  for (const node of graph.nodes) {
    const spot = positions.get(node.id);
    if (!spot || (node.pinned && !all)) {
      continue;
    }
    if (node.x !== null && node.y !== null) {
      before.push({ nodeId: node.id, x: node.x, y: node.y, pinned: node.pinned });
    }
    changed.push({ nodeId: node.id, x: Math.round(spot.x), y: Math.round(spot.y), pinned: false });
  }
  return { positions: changed, before };
}
