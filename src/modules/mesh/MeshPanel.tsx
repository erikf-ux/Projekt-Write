/**
 * The Mesh as a main view in its own tab (section 11, 15): views,
 * breadcrumb, and the keyboard (section 6: "Alle Funktionen per Tastatur").
 *
 * Keys on the canvas: arrows move the selection to the nearest node in
 * that direction, Enter opens (documents, media) or shows a subnet, Space
 * opens or closes a cluster in place, F shows the Focus View, C starts a
 * connection (choose the other end, Enter), E selects the next connection
 * of the node, Entf removes a selected confirmed connection, Backspace or
 * Escape goes back, + and - zoom, 0 fits, Ctrl+Z undoes.
 */
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { type MessageKey, useT } from '@/i18n';
import { useWorkspace } from '@/modules/documents/workspaceStore';
import { useStructure } from '@/modules/structure/structureStore';
import type { MeshNode } from '@/types/generated/MeshNode';
import type { MeshPosition } from '@/types/generated/MeshPosition';
import { UNASSIGNED, useMesh } from './meshStore';
import { SuggestionsPanel } from './SuggestionsPanel';
import { useSuggestions } from './suggestionStore';
import type { MeshCanvasControl } from './renderer';

const MeshCanvas = lazy(() => import('./MeshCanvas'));

type Direction = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

/** The nearest node in a direction, weighting the cross distance double. */
export function nearest(
  from: { x: number; y: number },
  direction: Direction,
  candidates: readonly { id: string; x: number; y: number }[],
): string | null {
  // First within 45 degrees of the arrow; only if none is there, anywhere
  // on that side.
  for (const cone of [true, false]) {
    const found = nearestOnSide(from, direction, candidates, cone);
    if (found) {
      return found;
    }
  }
  return null;
}

function nearestOnSide(
  from: { x: number; y: number },
  direction: Direction,
  candidates: readonly { id: string; x: number; y: number }[],
  cone: boolean,
): string | null {
  let best: string | null = null;
  let bestScore = Infinity;
  for (const node of candidates) {
    const dx = node.x - from.x;
    const dy = node.y - from.y;
    const along =
      direction === 'ArrowRight'
        ? dx
        : direction === 'ArrowLeft'
          ? -dx
          : direction === 'ArrowDown'
            ? dy
            : -dy;
    const across = direction === 'ArrowRight' || direction === 'ArrowLeft' ? Math.abs(dy) : Math.abs(dx);
    if (along <= 1 || (cone && across > along)) {
      continue;
    }
    const score = along + across * 2;
    if (score < bestScore) {
      bestScore = score;
      best = node.id;
    }
  }
  return best;
}

export function MeshPanel({ hidden }: { hidden: boolean }) {
  const t = useT();
  const mesh = useMesh();
  const { graph, place, selected, selectedEdge } = mesh;
  const control = useRef<MeshCanvasControl | null>(null);
  const [connectFrom, setConnectFrom] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState('');
  const suggestionsOpen = useSuggestions((state) => state.open);
  const toggleSuggestions = useSuggestions((state) => state.toggle);

  // Shown again, or the structure changed (Explorer, undo): drawn anew.
  const structureSubnets = useStructure((state) => state.subnets);
  const structureElements = useStructure((state) => state.byElement);
  useEffect(() => {
    if (!hidden) {
      void useMesh.getState().load();
    }
  }, [hidden, mesh.generation, structureSubnets, structureElements]);

  const titleOf = useCallback(
    (node: MeshNode) =>
      node.kind === 'unassigned' ? t('mesh.unassigned') : node.title || t('document.untitled'),
    [t],
  );

  const byId = useMemo(() => new Map((graph?.nodes ?? []).map((node) => [node.id, node])), [graph]);

  const open = useCallback((node: MeshNode) => {
    if (node.kind === 'document') {
      void useWorkspace.getState().openDocument(node.id);
    } else if (node.kind === 'media') {
      useWorkspace.getState().openMedia(node.id);
    } else if (node.kind === 'subnet') {
      void useMesh.getState().show({ view: 'subnet', scopeId: node.id });
    } else {
      void useMesh.getState().toggle(UNASSIGNED);
    }
  }, []);

  const announce = useCallback(
    (id: string | null) => {
      const node = id ? byId.get(id) : undefined;
      setAnnouncement(
        node ? t(`mesh.announce.${node.kind}` as MessageKey, { name: titleOf(node), count: node.count }) : '',
      );
    },
    [byId, t, titleOf],
  );

  const selectOne = useCallback(
    (id: string | null) => {
      useMesh.getState().select(id ? [id] : []);
      announce(id);
      if (id) {
        control.current?.centerOn(id);
      }
    },
    [announce],
  );

  const onMoved = useCallback((positions: MeshPosition[], before: MeshPosition[]) => {
    void useMesh.getState().savePositions(positions, before);
  }, []);

  const onConnect = useCallback((a: string, b: string) => {
    void useMesh.getState().connect(a, b);
  }, []);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!graph) {
      return;
    }
    const current = selected[0] ?? null;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault();
      void mesh.undo();
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }
    switch (event.key) {
      case 'ArrowUp':
      case 'ArrowDown':
      case 'ArrowLeft':
      case 'ArrowRight': {
        event.preventDefault();
        const drawn = control.current?.positions() ?? [];
        const centers = drawn.map((node) => ({
          id: node.id,
          x: node.x + node.width / 2,
          y: node.y + node.height / 2,
        }));
        if (!current) {
          selectOne(graph.nodes[0]?.id ?? null);
          return;
        }
        const from = centers.find((node) => node.id === current);
        const next = from
          ? nearest(
              from,
              event.key,
              centers.filter((node) => node.id !== current),
            )
          : null;
        if (next) {
          selectOne(next);
        }
        return;
      }
      case 'Enter': {
        event.preventDefault();
        if (connectFrom && current && current !== connectFrom) {
          void mesh.connect(connectFrom, current);
          setConnectFrom(null);
          return;
        }
        const node = current ? byId.get(current) : undefined;
        if (node) {
          open(node);
        }
        return;
      }
      case ' ': {
        const node = current ? byId.get(current) : undefined;
        if (node && (node.kind === 'subnet' || node.kind === 'unassigned')) {
          event.preventDefault();
          void mesh.toggle(node.id);
        }
        return;
      }
      case 'f':
      case 'F': {
        const node = current ? byId.get(current) : undefined;
        if (node && node.kind !== 'unassigned') {
          event.preventDefault();
          void mesh.show({ view: 'focus', scopeId: node.id });
        }
        return;
      }
      case 'c':
      case 'C': {
        if (current && byId.get(current)?.kind !== 'unassigned') {
          event.preventDefault();
          setConnectFrom(current);
          setAnnouncement(t('mesh.connectStart'));
        }
        return;
      }
      case 'e':
      case 'E': {
        if (!current) {
          return;
        }
        event.preventDefault();
        const touching = graph.edges.filter((edge) => edge.source === current || edge.target === current);
        if (touching.length === 0) {
          return;
        }
        const index = touching.findIndex((edge) => edge.id === selectedEdge);
        const edge = touching[(index + 1) % touching.length];
        if (edge) {
          mesh.selectEdge(edge.id);
          useMesh.setState({ selected: [current] });
          setAnnouncement(
            t(`mesh.edge.${edge.kind}` as MessageKey) + (edge.connectionId ? '' : ` – ${t('mesh.derived')}`),
          );
        }
        return;
      }
      case 'Delete': {
        const edge = graph.edges.find((candidate) => candidate.id === selectedEdge);
        if (edge?.connectionId) {
          event.preventDefault();
          void mesh.disconnect(edge.connectionId);
        } else if (edge) {
          setAnnouncement(t('mesh.derivedCannotDelete'));
        }
        return;
      }
      case 'Escape':
        if (connectFrom) {
          setConnectFrom(null);
          setAnnouncement('');
          return;
        }
        void mesh.back();
        return;
      case 'Backspace':
        void mesh.back();
        return;
      case 'l':
      case 'L':
        event.preventDefault();
        void mesh.autoLayout(event.shiftKey);
        return;
      case '+':
        control.current?.zoomBy(1.25);
        return;
      case '-':
        control.current?.zoomBy(0.8);
        return;
      case '0':
        control.current?.fit();
        return;
      default:
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid="mesh">
      <div className="flex h-9 shrink-0 items-center gap-2 border-b border-muted/40 px-2 text-xs">
        <nav
          aria-label={t('mesh.breadcrumb')}
          className="flex min-w-0 items-center gap-1"
          data-testid="mesh-breadcrumb"
        >
          <button
            type="button"
            className="rounded px-1.5 py-0.5 hover:bg-muted/30"
            aria-current={place.view === 'global' ? 'page' : undefined}
            onClick={() => void mesh.show({ view: 'global', scopeId: '' })}
          >
            {t('mesh.global')}
          </button>
          {(graph?.breadcrumb ?? []).map((crumb) => (
            <span key={crumb.id} className="flex min-w-0 items-center gap-1">
              <span aria-hidden className="text-muted">
                ›
              </span>
              <button
                type="button"
                className="truncate rounded px-1.5 py-0.5 hover:bg-muted/30"
                onClick={() =>
                  void mesh.show(
                    place.view === 'focus'
                      ? { view: 'focus', scopeId: crumb.id }
                      : { view: 'subnet', scopeId: crumb.id },
                  )
                }
              >
                {place.view === 'focus' ? t('mesh.focusOf', { name: crumb.title }) : crumb.title}
              </button>
            </span>
          ))}
        </nav>
        <span className="flex-1" />
        {place.view === 'focus' ? (
          <label className="flex items-center gap-1">
            <input
              type="checkbox"
              checked={mesh.depth === 2}
              data-testid="mesh-depth"
              onChange={(event) => void mesh.setDepth(event.target.checked ? 2 : 1)}
            />
            {t('mesh.depth2')}
          </label>
        ) : null}
        <button
          type="button"
          className="rounded px-1.5 py-0.5 hover:bg-muted/30"
          data-testid="mesh-suggestions-toggle"
          aria-pressed={suggestionsOpen}
          onClick={toggleSuggestions}
        >
          {t('mesh.suggestions')}
        </button>
        <button
          type="button"
          className="rounded px-1.5 py-0.5 hover:bg-muted/30 disabled:opacity-50"
          data-testid="mesh-layout"
          disabled={mesh.layouting || !graph}
          onClick={() => void mesh.autoLayout(false)}
        >
          {t('mesh.layout')}
        </button>
        {graph && graph.hidden > 0 ? (
          <span className="text-warning" data-testid="mesh-hidden" role="status">
            {t('mesh.hidden', { count: graph.hidden })}
          </span>
        ) : null}
      </div>
      <div className="flex min-h-0 flex-1">
        <div
          className="relative min-h-0 min-w-0 flex-1 outline-none focus-visible:ring-2 focus-visible:ring-accent"
          tabIndex={0}
          role="application"
          aria-label={t('mesh.canvas')}
          aria-describedby="pw-mesh-help"
          data-testid="mesh-canvas"
          onKeyDown={onKeyDown}
        >
          {graph ? (
            <Suspense fallback={<p className="p-4 text-sm text-muted">{t('mesh.loading')}</p>}>
              <MeshCanvas
                graph={graph}
                selected={selected}
                selectedEdge={selectedEdge}
                titleOf={titleOf}
                onSelect={(ids) => {
                  mesh.select(ids);
                  announce(ids[0] ?? null);
                }}
                onSelectEdge={mesh.selectEdge}
                onOpen={open}
                onToggle={(id) => void mesh.toggle(id)}
                onMoved={onMoved}
                onConnect={onConnect}
                onDropOnSubnet={(elementId, subnetId) => {
                  void useStructure
                    .getState()
                    .assign(elementId, subnetId)
                    .then(() => useMesh.getState().load());
                }}
                onDropElement={(elementId) => void mesh.show({ view: 'focus', scopeId: elementId })}
                onReady={(next) => {
                  control.current = next;
                }}
              />
            </Suspense>
          ) : (
            <p className="p-4 text-sm text-muted">
              {mesh.error ? t(mesh.error.messageKey as MessageKey) : t('mesh.loading')}
            </p>
          )}
        </div>
        {suggestionsOpen ? <SuggestionsPanel /> : null}
      </div>
      <p id="pw-mesh-help" className="sr-only">
        {t('mesh.help')}
      </p>
      <p className="sr-only" aria-live="polite" data-testid="mesh-announce">
        {announcement}
      </p>
      {connectFrom ? (
        <p className="border-t border-muted/40 px-2 py-1 text-xs" role="status" data-testid="mesh-connecting">
          {t('mesh.connectStart')}
        </p>
      ) : null}
    </div>
  );
}
