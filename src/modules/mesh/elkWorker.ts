/**
 * Automatic layout of the Mesh in a web worker (section 15: "ELK.js läuft
 * im Web Worker"). Receives a graph with node sizes and hulls, answers
 * with positions (relative to the hull, like React Flow's `parentId`).
 */
import ELK from 'elkjs/lib/elk.bundled.js';
import type { ElkNode } from 'elkjs/lib/elk-api';

export interface LayoutRequest {
  id: number;
  nodes: { id: string; parentId: string | null; width: number; height: number; open: boolean }[];
  edges: { id: string; source: string; target: string }[];
}

export interface LayoutResponse {
  id: number;
  positions: { id: string; x: number; y: number }[];
  error?: string;
}

const elk = new ELK();

function build(request: LayoutRequest): ElkNode {
  const byParent = new Map<string | null, LayoutRequest['nodes']>();
  for (const node of request.nodes) {
    const list = byParent.get(node.parentId) ?? [];
    list.push(node);
    byParent.set(node.parentId, list);
  }
  const make = (node: LayoutRequest['nodes'][number]): ElkNode => {
    const children = byParent.get(node.id) ?? [];
    if (node.open && children.length > 0) {
      return {
        id: node.id,
        layoutOptions: {
          'elk.algorithm': 'org.eclipse.elk.rectpacking',
          'elk.padding': '[top=40,left=20,bottom=20,right=20]',
          'elk.spacing.nodeNode': '16',
        },
        children: children.map(make),
      };
    }
    return { id: node.id, width: node.width, height: node.height };
  };
  const parentOf = new Map(request.nodes.map((node) => [node.id, node.parentId]));
  return {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'org.eclipse.elk.layered',
      'elk.direction': 'RIGHT',
      'elk.hierarchyHandling': 'SEPARATE_CHILDREN',
      'elk.spacing.nodeNode': '24',
      'elk.layered.spacing.nodeNodeBetweenLayers': '60',
      'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    },
    children: (byParent.get(null) ?? []).map(make),
    // Only edges between top-level nodes steer the layered layout; the
    // content of a hull is packed.
    edges: request.edges
      .filter((edge) => parentOf.get(edge.source) === null && parentOf.get(edge.target) === null)
      .map((edge) => ({ id: edge.id, sources: [edge.source], targets: [edge.target] })),
  };
}

function collect(node: ElkNode, into: LayoutResponse['positions']) {
  for (const child of node.children ?? []) {
    into.push({ id: child.id, x: child.x ?? 0, y: child.y ?? 0 });
    collect(child, into);
  }
}

self.onmessage = (event: MessageEvent<LayoutRequest>) => {
  const request = event.data;
  elk
    .layout(build(request))
    .then((result) => {
      const positions: LayoutResponse['positions'] = [];
      collect(result, positions);
      self.postMessage({ id: request.id, positions } satisfies LayoutResponse);
    })
    .catch((error: unknown) => {
      self.postMessage({ id: request.id, positions: [], error: String(error) } satisfies LayoutResponse);
    });
};
