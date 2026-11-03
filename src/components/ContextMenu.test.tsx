import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ContextMenu, isContextMenuKey } from './ContextMenu';

describe('ContextMenu', () => {
  it('is operated with the keyboard and returns the focus', () => {
    const trigger = document.createElement('button');
    document.body.append(trigger);
    trigger.focus();
    const open = vi.fn();
    const rename = vi.fn();
    const onClose = vi.fn();
    const { unmount } = render(
      <ContextMenu
        x={10}
        y={10}
        label="Dokument"
        onClose={onClose}
        items={[
          { id: 'open', label: 'Öffnen', onSelect: open },
          { id: 'rename', label: 'Umbenennen', shortcut: 'F2', onSelect: rename },
        ]}
      />,
    );
    const items = screen.getAllByRole('menuitem');
    expect(document.activeElement).toBe(items[0]);
    fireEvent.keyDown(items[0] as HTMLElement, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(items[1]);
    fireEvent.keyDown(items[1] as HTMLElement, { key: 'ArrowDown' });
    // Wraps around.
    expect(document.activeElement).toBe(items[0]);
    fireEvent.keyDown(items[0] as HTMLElement, { key: 'End' });
    fireEvent.keyDown(items[1] as HTMLElement, { key: 'Enter' });
    expect(rename).toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it('closes with Escape without selecting', () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    render(
      <ContextMenu
        x={0}
        y={0}
        label="Tab"
        onClose={onClose}
        items={[{ id: 'close', label: 'Schließen', onSelect }]}
      />,
    );
    fireEvent.keyDown(screen.getByRole('menuitem'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('recognises the context menu key and Shift+F10', () => {
    expect(isContextMenuKey({ key: 'ContextMenu', shiftKey: false })).toBe(true);
    expect(isContextMenuKey({ key: 'F10', shiftKey: true })).toBe(true);
    expect(isContextMenuKey({ key: 'F10', shiftKey: false })).toBe(false);
  });
});
