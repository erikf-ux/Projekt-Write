/**
 * The renderer of the Mesh: React Flow (section 7, ADR 0022). It sits
 * behind {@link MeshRendererProps}, so another renderer (Sigma.js) could
 * replace it. Loaded only when the Mesh is opened.
 *
 * Section 15: memoised node components, no shadows or animations, parents
 * before their children; React Flow draws only what is in view.
 */
import { createContext, memo, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import {
  Background,
  type Connection,
  Controls,
  type Edge,
  Handle,
  MarkerType,
  MiniMap,
  type Node,
  type NodeProps,
  type OnNodeDrag,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useNodesState,
  useOnViewportChange,
  useReactFlow,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import {
  ChevronDown,
  ChevronRight,
  FileText,
  FileType,
  Image,
  Network,
  Paperclip,
  Shapes,
} from 'lucide-react';
import { type MessageKey, useT } from '@/i18n';
import { EXPLORER_DRAG_TYPE } from '@/modules/documents/Explorer';
import { colorValue } from '@/modules/structure/tree';
import type { MeshEdge } from '@/types/generated/MeshEdge';
import type { MeshNode } from '@/types/generated/MeshNode';
import type { MeshPosition } from '@/types/generated/MeshPosition';
import { place } from './layout';
import type { MeshRendererProps } from './renderer';

interface NodeData extends Record<string, unknown> {
  node: MeshNode;
  selected: boolean;
  onToggle: (id: string) => void;
  label: string;
}

type MeshFlowNode = Node<NodeData>;

function nodeColor(node: MeshNode): string {
  if (node.archived) {
    return 'var(--pw-color-cluster-grey)';
  }
  if (node.kind === 'media') {
    return 'var(--pw-color-cluster-orange)';
  }
  if (node.color) {
    return colorValue(node.color);
  }
  return 'var(--pw-color-accent)';
}

function ElementIcon({ node }: { node: MeshNode }) {
  const size = 14;
  if (node.kind === 'document') {
    return <FileText size={size} aria-hidden />;
  }
  if (node.mediaKind === 'image') {
    return <Image size={size} aria-hidden />;
  }
  if (node.mediaKind === 'pdf') {
    return <FileType size={size} aria-hidden />;
  }
  return <Paperclip size={size} aria-hidden />;
}

const handleStyle = { width: 8, height: 8, background: 'var(--pw-color-muted)', border: 'none' };
const quietHandle = { ...handleStyle, opacity: 0 };

/**
 * Below this zoom the text of a node cannot be read: nodes are drawn as
 * plain coloured boxes, which keeps zooming and panning with many nodes
 * fluid (section 15: "verzichte bei vielen Knoten auf aufwendige …").
 */
export const DETAIL_ZOOM = 0.45;
/** Around the threshold the level stays as it is (no flicker, no redraw of every node on each step). */
const DETAIL_HYSTERESIS = 0.05;

const DetailContext = createContext(false);

function useDetailed(): boolean {
  return useContext(DetailContext);
}

const ElementNode = memo(function ElementNode({ data }: NodeProps<MeshFlowNode>) {
  const { node, selected, label } = data;
  const detailed = useDetailed();
  if (!detailed) {
    return (
      <div
        className="h-full w-full"
        style={{
          // Opaque and square: no blending and no rounded edges to paint.
          background: nodeColor(node),
          outline: selected ? '4px solid var(--pw-color-text)' : undefined,
        }}
        data-mesh-node={node.id}
        data-kind={node.kind}
        data-selected={selected ? 'true' : undefined}
      >
        <Handle type="target" position={Position.Left} style={quietHandle} />
        <Handle type="source" position={Position.Right} style={quietHandle} />
      </div>
    );
  }
  return (
    <div
      className="flex h-full w-full items-center gap-2 overflow-hidden rounded-md border bg-surface px-2 text-xs text-text"
      style={{
        borderColor: nodeColor(node),
        borderWidth: selected ? 3 : 1.5,
        opacity: node.archived ? 0.75 : 1,
      }}
      data-mesh-node={node.id}
      data-kind={node.kind}
      data-selected={selected ? 'true' : undefined}
      title={label}
    >
      <Handle type="target" position={Position.Left} style={handleStyle} />
      <span style={{ color: nodeColor(node) }}>
        <ElementIcon node={node} />
      </span>
      <span className="truncate">{label}</span>
      <Handle type="source" position={Position.Right} style={handleStyle} />
    </div>
  );
});

const ClusterNode = memo(function ClusterNode({ data }: NodeProps<MeshFlowNode>) {
  const t = useT();
  const { node, selected, onToggle, label } = data;
  const color = nodeColor(node);
  const canOpen = node.kind === 'unassigned' || node.count > 0 || node.childSubnets > 0;
  return (
    <div
      className="flex h-full w-full items-center gap-2 rounded-lg border-2 bg-sidebar px-2 text-xs text-text"
      style={{
        borderColor: color,
        borderWidth: selected ? 4 : 2,
        borderStyle: node.kind === 'unassigned' ? 'dashed' : 'solid',
      }}
      data-mesh-node={node.id}
      data-kind={node.kind}
      data-open="false"
      data-selected={selected ? 'true' : undefined}
      title={label}
    >
      <Handle type="target" position={Position.Left} style={handleStyle} />
      {canOpen ? (
        <button
          type="button"
          className="nodrag rounded p-0.5 hover:bg-muted/30"
          aria-label={t('mesh.expand', { name: label })}
          data-testid="mesh-expand"
          onClick={(event) => {
            event.stopPropagation();
            onToggle(node.id);
          }}
        >
          <ChevronRight size={14} aria-hidden />
        </button>
      ) : null}
      <span style={{ color }}>
        {node.kind === 'unassigned' ? <Shapes size={16} aria-hidden /> : <Network size={16} aria-hidden />}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate font-semibold">{label}</span>
        <span className="block text-muted">{t('mesh.count', { count: node.count })}</span>
      </span>
      <Handle type="source" position={Position.Right} style={handleStyle} />
    </div>
  );
});

const HullNode = memo(function HullNode({ data }: NodeProps<MeshFlowNode>) {
  const t = useT();
  const detailed = useDetailed();
  const { node, selected, onToggle, label } = data;
  const color = nodeColor(node);
  return (
    <div
      className="h-full w-full rounded-xl border-2"
      style={{
        borderColor: color,
        borderWidth: selected ? 4 : 2,
        borderStyle: node.kind === 'unassigned' ? 'dashed' : 'solid',
        // A tinted fill is blended over a large area: only up close.
        ...(detailed ? { background: `color-mix(in srgb, ${color} 8%, transparent)` } : {}),
      }}
      data-mesh-node={node.id}
      data-kind={node.kind}
      data-open="true"
      data-selected={selected ? 'true' : undefined}
    >
      <div className="flex h-8 items-center gap-1 px-2 text-xs font-semibold text-text">
        <button
          type="button"
          className="nodrag rounded p-0.5 hover:bg-muted/30"
          aria-label={t('mesh.collapse', { name: label })}
          data-testid="mesh-collapse"
          onClick={(event) => {
            event.stopPropagation();
            onToggle(node.id);
          }}
        >
          <ChevronDown size={14} aria-hidden />
        </button>
        <span style={{ color }}>
          <Network size={14} aria-hidden />
        </span>
        <span className="truncate">{label}</span>
        <span className="font-normal text-muted">{t('mesh.count', { count: node.count })}</span>
      </div>
    </div>
  );
});

const nodeTypes = { element: ElementNode, cluster: ClusterNode, hull: HullNode };

function edgeStyle(edge: MeshEdge, selected: boolean): Partial<Edge> {
  const stroke =
    edge.kind === 'reference'
      ? 'var(--pw-color-muted)'
      : edge.kind === 'membership'
        ? 'var(--pw-color-cluster-violet)'
        : 'var(--pw-color-accent)';
  return {
    style: {
      stroke,
      strokeWidth: (selected ? 3 : 1.5) + Math.min(3, Math.log2(edge.count)),
      ...(edge.kind === 'similarity' ? { strokeDasharray: '6 4' } : {}),
      ...(edge.kind === 'membership' ? { strokeDasharray: '2 4' } : {}),
    },
    ...(edge.directed
      ? { markerEnd: { type: MarkerType.ArrowClosed, color: stroke, width: 16, height: 16 } }
      : {}),
    ...(edge.count > 1 ? { label: String(edge.count) } : {}),
  };
}

function Canvas({
  graph,
  selected,
  selectedEdge,
  titleOf,
  onSelect,
  onSelectEdge,
  onOpen,
  onToggle,
  onMoved,
  onConnect,
  onDropOnSubnet,
  onDropElement,
  onReady,
  onNodeMenu,
  onEdgeMenu,
}: MeshRendererProps) {
  const t = useT();
  const flow = useReactFlow<MeshFlowNode>();
  const placed = useMemo(() => place(graph), [graph]);
  const selectedSet = useMemo(() => new Set(selected), [selected]);

  const initialNodes = useMemo<MeshFlowNode[]>(
    () =>
      graph.nodes.map((node) => {
        const spot = placed.get(node.id);
        const isElement = node.kind === 'document' || node.kind === 'media';
        const width = spot?.width ?? 0;
        const height = spot?.height ?? 0;
        return {
          id: node.id,
          type: isElement ? 'element' : node.open ? 'hull' : 'cluster',
          position: { x: spot?.x ?? 0, y: spot?.y ?? 0 },
          ...(node.parentId ? { parentId: node.parentId } : {}),
          width,
          height,
          style: { width, height },
          zIndex: node.open ? 0 : 1,
          selectable: true,
          data: {
            node,
            selected: selectedSet.has(node.id),
            onToggle,
            label: titleOf(node),
          },
          ariaLabel: titleOf(node),
        };
      }),
    // selection is applied below without rebuilding the nodes
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [graph, placed, onToggle, titleOf],
  );
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  useEffect(() => {
    setNodes(initialNodes);
  }, [initialNodes, setNodes]);
  useEffect(() => {
    setNodes((current) =>
      current.map((node) => {
        const isSelected = selectedSet.has(node.id);
        return node.data.selected === isSelected
          ? node
          : { ...node, data: { ...node.data, selected: isSelected } };
      }),
    );
  }, [selectedSet, setNodes]);

  const edges = useMemo<Edge[]>(
    () =>
      graph.edges.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        selectable: true,
        focusable: true,
        ariaLabel: t(`mesh.edge.${edge.kind}` as MessageKey),
        data: { edge },
        // Straight lines: much cheaper to draw than curves with many edges.
        type: 'straight',
        ...edgeStyle(edge, edge.id === selectedEdge),
      })),
    [graph.edges, selectedEdge, t],
  );

  const fitted = useRef<string>('');
  useEffect(() => {
    const key = `${graph.view}:${graph.scopeId}`;
    if (fitted.current !== key) {
      fitted.current = key;
      requestAnimationFrame(() => {
        void flow.fitView({ padding: 0.1, duration: 0 });
      });
    }
  }, [graph.view, graph.scopeId, flow]);

  useEffect(() => {
    onReady({
      zoomBy: (factor) => {
        void flow.zoomTo(flow.getZoom() * factor, { duration: 0 });
      },
      fit: () => {
        void flow.fitView({ padding: 0.1, duration: 0 });
      },
      centerOn: (id) => {
        const node = flow.getInternalNode(id);
        if (node) {
          const { x, y } = node.internals.positionAbsolute;
          void flow.setCenter(x + (node.measured.width ?? 0) / 2, y + (node.measured.height ?? 0) / 2, {
            zoom: Math.max(flow.getZoom(), 0.8),
            duration: 0,
          });
        }
      },
      positions: () =>
        flow.getNodes().map((node) => {
          const internal = flow.getInternalNode(node.id);
          return {
            id: node.id,
            x: internal?.internals.positionAbsolute.x ?? node.position.x,
            y: internal?.internals.positionAbsolute.y ?? node.position.y,
            width: internal?.measured.width ?? 0,
            height: internal?.measured.height ?? 0,
          };
        }),
    });
  }, [flow, onReady]);

  const dragStart = useRef<MeshPosition[]>([]);
  const onNodeDragStart: OnNodeDrag<MeshFlowNode> = useCallback((_, __, dragged) => {
    dragStart.current = dragged.map((node) => ({
      nodeId: node.id,
      x: node.position.x,
      y: node.position.y,
      pinned: node.data.node.pinned,
    }));
  }, []);
  /** The innermost subnet under a rectangle (smallest area first). */
  const subnetAt = useCallback(
    (rect: { x: number; y: number; width: number; height: number }, except: string | null) => {
      const hits = flow
        .getIntersectingNodes(rect, true)
        .filter((node) => node.data.node.kind === 'subnet' && node.id !== except);
      hits.sort(
        (a, b) =>
          (a.measured?.width ?? 0) * (a.measured?.height ?? 0) -
          (b.measured?.width ?? 0) * (b.measured?.height ?? 0),
      );
      return hits[0] ?? null;
    },
    [flow],
  );

  const onNodeDragStop: OnNodeDrag<MeshFlowNode> = useCallback(
    (_, __, dragged) => {
      // One element dropped on a subnet other than its own hull: assigned.
      const only = dragged.length === 1 ? dragged[0] : undefined;
      if (only && (only.data.node.kind === 'document' || only.data.node.kind === 'media')) {
        const internal = flow.getInternalNode(only.id);
        const center = internal
          ? {
              x: internal.internals.positionAbsolute.x + (internal.measured.width ?? 0) / 2,
              y: internal.internals.positionAbsolute.y + (internal.measured.height ?? 0) / 2,
            }
          : null;
        const target = center ? subnetAt({ ...center, width: 1, height: 1 }, only.id) : null;
        if (target && target.id !== only.data.node.parentId) {
          onDropOnSubnet(only.id, target.id);
          return;
        }
      }
      const before = dragStart.current.filter((position) =>
        graph.nodes.some((node) => node.id === position.nodeId && node.x !== null),
      );
      onMoved(
        dragged.map((node) => ({ nodeId: node.id, x: node.position.x, y: node.position.y, pinned: true })),
        before,
      );
    },
    [flow, graph.nodes, onDropOnSubnet, onMoved, subnetAt],
  );

  const onDrop = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      const data = event.dataTransfer.getData(EXPLORER_DRAG_TYPE);
      if (!data) {
        return;
      }
      event.preventDefault();
      let item: { kind?: string; id?: string };
      try {
        item = JSON.parse(data) as { kind?: string; id?: string };
      } catch {
        return;
      }
      if (!item.id || (item.kind !== 'document' && item.kind !== 'media')) {
        return;
      }
      const point = flow.screenToFlowPosition({ x: event.clientX, y: event.clientY });
      const target = subnetAt({ ...point, width: 1, height: 1 }, null);
      if (target) {
        onDropOnSubnet(item.id, target.id);
      } else {
        onDropElement(item.id);
      }
    },
    [flow, onDropElement, onDropOnSubnet, subnetAt],
  );

  const many = graph.nodes.length > 300;
  // The level of detail changes only when the zoom passes the threshold
  // by more than the hysteresis: every node redraws then, once.
  const [detailed, setDetailed] = useState(() => flow.getZoom() >= DETAIL_ZOOM);
  useOnViewportChange({
    onChange: (viewport) => {
      setDetailed((current) =>
        current
          ? viewport.zoom >= DETAIL_ZOOM - DETAIL_HYSTERESIS
          : viewport.zoom >= DETAIL_ZOOM + DETAIL_HYSTERESIS,
      );
    },
  });

  return (
    <DetailContext.Provider value={detailed}>
      <div
        className="h-full w-full"
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes(EXPLORER_DRAG_TYPE)) {
            event.preventDefault();
            event.dataTransfer.dropEffect = 'link';
          }
        }}
        onDrop={onDrop}
      >
        <ReactFlow<MeshFlowNode>
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onNodeClick={(event, node) => {
            if (event.shiftKey) {
              onSelect(
                selectedSet.has(node.id) ? selected.filter((id) => id !== node.id) : [...selected, node.id],
              );
              return;
            }
            const kind = node.data.node.kind;
            if (kind === 'subnet' && !node.data.node.open) {
              onOpen(node.data.node);
              return;
            }
            onSelect([node.id]);
          }}
          onNodeDoubleClick={(_, node) => {
            onOpen(node.data.node);
          }}
          onEdgeClick={(_, edge) => {
            onSelectEdge(edge.id);
          }}
          onNodeContextMenu={(event, node) => {
            event.preventDefault();
            onSelect([node.id]);
            onNodeMenu(node.data.node, event.clientX, event.clientY);
          }}
          onEdgeContextMenu={(event, edge) => {
            event.preventDefault();
            const found = graph.edges.find((candidate) => candidate.id === edge.id);
            if (found) {
              onSelectEdge(edge.id);
              onEdgeMenu(found, event.clientX, event.clientY);
            }
          }}
          onPaneClick={() => {
            onSelect([]);
          }}
          onConnect={(connection: Connection) => {
            if (connection.source && connection.target) {
              onConnect(connection.source, connection.target);
            }
          }}
          onNodeDragStart={onNodeDragStart}
          onNodeDragStop={onNodeDragStop}
          onlyRenderVisibleElements={false}
          minZoom={0.05}
          maxZoom={2.5}
          nodesFocusable={false}
          edgesFocusable={false}
          deleteKeyCode={null}
          selectionKeyCode={null}
          multiSelectionKeyCode={null}
          zoomOnDoubleClick={false}
          proOptions={{ hideAttribution: true }}
          colorMode="system"
          aria-label={t('mesh.canvas')}
        >
          <Background gap={24} size={1} />
          <Controls showInteractive={false} position="bottom-right" />
          <MiniMap pannable zoomable position="bottom-left" nodeStrokeWidth={many ? 0 : 2} />
        </ReactFlow>
      </div>
    </DetailContext.Provider>
  );
}

/** The renderer, with its own React Flow state. */
export function MeshCanvas(props: MeshRendererProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

export default MeshCanvas;
