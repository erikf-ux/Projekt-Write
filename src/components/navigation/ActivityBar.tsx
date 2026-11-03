/**
 * Activity bar (section 11, 48 px): switches between the main views and
 * side bars. Phase 3 has the Explorer and the editor; search and Mesh
 * appear with their phases (ADR 0011).
 */
import { IconToolbar, type ToolbarItem } from '@/components/toolbar/IconToolbar';

interface ActivityBarProps {
  label: string;
  items: readonly ToolbarItem[];
  bottom: readonly ToolbarItem[];
}

export function ActivityBar({ label, items, bottom }: ActivityBarProps) {
  return (
    <nav
      aria-label={label}
      data-testid="activity-bar"
      className="flex w-12 shrink-0 flex-col items-center justify-between border-r border-muted/40 bg-sidebar py-2"
    >
      <IconToolbar label={label} items={items} orientation="vertical" />
      <IconToolbar label={label} items={bottom} orientation="vertical" />
    </nav>
  );
}
