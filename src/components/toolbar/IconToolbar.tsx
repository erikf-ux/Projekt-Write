/**
 * A row or column of icon buttons (ARIA toolbar pattern): one tab stop,
 * arrow keys move between the buttons, Home/End jump. Every button has a
 * label for screen readers and a tooltip with its shortcut.
 */
import type { LucideIcon } from 'lucide-react';
import { useRef, useState, type KeyboardEvent } from 'react';

export interface ToolbarItem {
  id: string;
  icon: LucideIcon;
  label: string;
  shortcut?: string | undefined;
  /** For toggles: whether the function is on. */
  pressed?: boolean | undefined;
  disabled?: boolean | undefined;
  onSelect: () => void;
}

interface IconToolbarProps {
  label: string;
  items: readonly ToolbarItem[];
  orientation: 'horizontal' | 'vertical';
  className?: string;
  testId?: string;
}

export function IconToolbar({ label, items, orientation, className = '', testId }: IconToolbarProps) {
  const [current, setCurrent] = useState(0);
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const enabled = items.map((item, index) => (item.disabled ? -1 : index)).filter((index) => index >= 0);
  const stop = enabled.includes(current) ? current : (enabled[0] ?? 0);

  const move = (to: number) => {
    setCurrent(to);
    buttons.current[to]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const next = orientation === 'horizontal' ? 'ArrowRight' : 'ArrowDown';
    const previous = orientation === 'horizontal' ? 'ArrowLeft' : 'ArrowUp';
    const position = enabled.indexOf(stop);
    let target: number | undefined;
    if (event.key === next) {
      target = enabled[(position + 1) % enabled.length];
    } else if (event.key === previous) {
      target = enabled[(position - 1 + enabled.length) % enabled.length];
    } else if (event.key === 'Home') {
      target = enabled[0];
    } else if (event.key === 'End') {
      target = enabled[enabled.length - 1];
    }
    if (target !== undefined) {
      event.preventDefault();
      move(target);
    }
  };

  return (
    <div
      role="toolbar"
      aria-label={label}
      aria-orientation={orientation}
      data-testid={testId}
      className={`flex ${orientation === 'horizontal' ? 'flex-row items-center' : 'flex-col items-center'} gap-1 ${className}`}
      onKeyDown={onKeyDown}
    >
      {items.map((item, index) => {
        const Icon = item.icon;
        const title = item.shortcut ? `${item.label} (${item.shortcut})` : item.label;
        return (
          <button
            key={item.id}
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            tabIndex={index === stop ? 0 : -1}
            aria-label={item.label}
            aria-pressed={item.pressed}
            title={title}
            disabled={item.disabled}
            data-command={item.id}
            className={`inline-flex size-8 shrink-0 items-center justify-center rounded-pw disabled:opacity-40 ${
              item.pressed ? 'bg-surface text-accent' : 'text-muted hover:bg-surface hover:text-fg'
            }`}
            onFocus={() => {
              setCurrent(index);
            }}
            onClick={item.onSelect}
          >
            <Icon aria-hidden="true" className="size-4" />
          </button>
        );
      })}
    </div>
  );
}
