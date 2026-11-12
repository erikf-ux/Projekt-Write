/**
 * What the Mesh view needs from a renderer (section 7: "Der Renderer liegt
 * hinter einer Schnittstelle"). React Flow implements it (MeshCanvas.tsx);
 * Sigma.js would be the fallback (ADR 0022).
 */
import type { MeshEdge } from '@/types/generated/MeshEdge';
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshNode } from '@/types/generated/MeshNode';
import type { MeshPosition } from '@/types/generated/MeshPosition';

/** A drawn node, in absolute coordinates of the canvas. */
export interface DrawnNode {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** What the view can ask of a drawn canvas. */
export interface MeshCanvasControl {
  zoomBy: (factor: number) => void;
  fit: () => void;
  centerOn: (id: string) => void;
  positions: () => DrawnNode[];
}

export interface MeshRendererProps {
  graph: MeshGraph;
  selected: readonly string[];
  selectedEdge: string | null;
  /** A tag to highlight: elements without it are dimmed (section 15). */
  highlightTag: string | null;
  titleOf: (node: MeshNode) => string;
  onSelect: (ids: readonly string[]) => void;
  onSelectEdge: (id: string | null) => void;
  /** Double click, Enter: open a document or medium; show a subnet. */
  onOpen: (node: MeshNode) => void;
  /** Open or close a cluster in place. */
  onToggle: (id: string) => void;
  /** Nodes moved by hand (now pinned), and where they were before. */
  onMoved: (positions: MeshPosition[], before: MeshPosition[]) => void;
  /** A connection drawn from a handle. */
  onConnect: (a: string, b: string) => void;
  /** A node, or an element from the Explorer, dropped on a subnet. */
  onDropOnSubnet: (elementId: string, subnetId: string) => void;
  /** An element from the Explorer dropped beside every subnet. */
  onDropElement: (elementId: string) => void;
  onReady: (control: MeshCanvasControl) => void;
  /** Right click on a node or an edge, at screen coordinates. */
  onNodeMenu: (node: MeshNode, x: number, y: number) => void;
  onEdgeMenu: (edge: MeshEdge, x: number, y: number) => void;
}
