/**
 * The side bar with a resize handle (section 11: "Anpassbare Seitenleisten:
 * Breite ändern, ein- und ausblenden"). The handle is a focusable separator:
 * arrow keys change the width by 16 px, Home/End jump to the limits.
 */
import { useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from 'react';
import { useT } from '@/i18n';

export const MIN_SIDEBAR_WIDTH = 160;
export const MAX_SIDEBAR_WIDTH = 640;
const STEP = 16;

interface SidebarProps {
  width: number;
  /** Called while dragging (preview) and once at the end (save). */
  onResize: (width: number, final: boolean) => void;
  children: ReactNode;
}

function clamp(width: number): number {
  return Math.round(Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, width)));
}

export function Sidebar({ width, onResize, children }: SidebarProps) {
  const t = useT();
  const [dragging, setDragging] = useState(false);
  const start = useRef({ x: 0, width });

  const onKeyDown = (event: KeyboardEvent) => {
    let next: number | null = null;
    if (event.key === 'ArrowLeft') next = width - STEP;
    else if (event.key === 'ArrowRight') next = width + STEP;
    else if (event.key === 'Home') next = MIN_SIDEBAR_WIDTH;
    else if (event.key === 'End') next = MAX_SIDEBAR_WIDTH;
    if (next !== null) {
      event.preventDefault();
      onResize(clamp(next), true);
    }
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    start.current = { x: event.clientX, width };
    setDragging(true);
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (dragging) {
      onResize(clamp(start.current.width + event.clientX - start.current.x), false);
    }
  };
  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (dragging) {
      setDragging(false);
      onResize(clamp(start.current.width + event.clientX - start.current.x), true);
    }
  };

  return (
    <aside className="relative flex h-full shrink-0 bg-sidebar" style={{ width }} data-testid="sidebar">
      <div className="min-w-0 flex-1">{children}</div>
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label={t('sidebar.resize')}
        aria-valuenow={width}
        aria-valuemin={MIN_SIDEBAR_WIDTH}
        aria-valuemax={MAX_SIDEBAR_WIDTH}
        tabIndex={0}
        data-testid="sidebar-resize"
        className={`absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize outline-none focus-visible:bg-accent ${
          dragging ? 'bg-accent' : 'hover:bg-accent/60'
        }`}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
      />
    </aside>
  );
}
