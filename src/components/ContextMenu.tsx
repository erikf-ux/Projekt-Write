/**
 * Context menu (section 11). Opened with the right mouse button, the
 * context menu key or `Shift + F10`; operated with the arrow keys, Enter
 * and Escape (ARIA menu pattern). The focus returns to where it came from.
 */
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';

export interface ContextMenuItem {
  id: string;
  label: string;
  /** Shortcut as shown to the user. */
  shortcut?: string | undefined;
  onSelect: () => void;
}

export interface ContextMenuState {
  x: number;
  y: number;
  items: readonly ContextMenuItem[];
  label: string;
}

interface ContextMenuProps extends ContextMenuState {
  onClose: () => void;
}

export function ContextMenu({ x, y, items, label, onClose }: ContextMenuProps) {
  const menu = useRef<HTMLUListElement>(null);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState({ left: x, top: y });

  // Keep the menu inside the window.
  useLayoutEffect(() => {
    const element = menu.current;
    if (!element) {
      return;
    }
    const { width, height } = element.getBoundingClientRect();
    setPosition({
      left: Math.max(4, Math.min(x, window.innerWidth - width - 4)),
      top: Math.max(4, Math.min(y, window.innerHeight - height - 4)),
    });
  }, [x, y]);

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      previous?.focus();
    };
  }, []);

  useEffect(() => {
    menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]')[active]?.focus();
  }, [active]);

  const select = (item: ContextMenuItem | undefined) => {
    if (item) {
      onClose();
      item.onSelect();
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const last = items.length - 1;
    switch (event.key) {
      case 'ArrowDown':
        setActive((index) => (index >= last ? 0 : index + 1));
        break;
      case 'ArrowUp':
        setActive((index) => (index <= 0 ? last : index - 1));
        break;
      case 'Home':
        setActive(0);
        break;
      case 'End':
        setActive(last);
        break;
      case 'Enter':
      case ' ':
        select(items[active]);
        break;
      case 'Escape':
      case 'Tab':
        onClose();
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  };

  return (
    <div
      className="fixed inset-0 z-50"
      onMouseDown={onClose}
      onContextMenu={(event: MouseEvent) => {
        event.preventDefault();
        onClose();
      }}
    >
      <ul
        ref={menu}
        role="menu"
        aria-label={label}
        data-testid="context-menu"
        className="text-ui fixed min-w-48 rounded-pw border border-muted bg-sidebar py-1 text-fg shadow-lg"
        style={{ left: position.left, top: position.top }}
        onMouseDown={(event) => {
          event.stopPropagation();
        }}
        onKeyDown={onKeyDown}
      >
        {items.map((item, index) => (
          <li
            key={item.id}
            role="menuitem"
            tabIndex={index === active ? 0 : -1}
            data-item={item.id}
            className="flex cursor-default items-center justify-between gap-6 px-3 py-1 outline-none focus:bg-accent focus:text-surface"
            onMouseEnter={() => {
              setActive(index);
            }}
            onClick={() => {
              select(item);
            }}
          >
            <span>{item.label}</span>
            {item.shortcut ? <kbd className="font-sans">{item.shortcut}</kbd> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Position for a context menu opened from the keyboard: below the element. */
export function positionBelow(element: Element): { x: number; y: number } {
  const rect = element.getBoundingClientRect();
  return { x: rect.left + 8, y: rect.bottom };
}

/** Whether a key event asks for the context menu (menu key or Shift+F10). */
export function isContextMenuKey(event: { key: string; shiftKey: boolean }): boolean {
  return event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10');
}
