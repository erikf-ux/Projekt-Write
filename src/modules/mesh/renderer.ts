/**
 * What the Mesh view needs from a renderer (section 7: "Der Renderer liegt
 * hinter einer Schnittstelle"). React Flow implements it (MeshCanvas.tsx);
 * Sigma.js would be the fallback (ADR 0022).
 */
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
  onReady: (control: MeshCanvasControl) => void;
}
