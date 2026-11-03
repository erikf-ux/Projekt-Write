import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '@/i18n';
import { CommandPalette, filterEntries, type PaletteEntry } from './CommandPalette';

const entries: PaletteEntry[] = [
  { id: 'document.new', title: 'Neues Dokument', category: 'Dokument', shortcut: 'Strg+N' },
  { id: 'focus.toggle', title: 'Fokusmodus', category: 'Ansicht', shortcut: 'F11' },
  { id: 'zoom.in', title: 'Vergrößern', category: 'Ansicht' },
];

describe('filterEntries', () => {
  it('matches every word in title or category, title prefixes first', () => {
    expect(filterEntries(entries, '').map((e) => e.id)).toEqual(['document.new', 'focus.toggle', 'zoom.in']);
    expect(filterEntries(entries, 'ansicht').map((e) => e.id)).toEqual(['focus.toggle', 'zoom.in']);
    expect(filterEntries(entries, 'ANSICHT fokus').map((e) => e.id)).toEqual(['focus.toggle']);
    expect(filterEntries(entries, 'dok').map((e) => e.id)).toEqual(['document.new']);
    expect(filterEntries(entries, 'xyz')).toEqual([]);
  });
});

describe('CommandPalette', () => {
  beforeEach(() => {
    useLocale.getState().setLocale('de');
  });

  it('focuses the input, filters, moves with the arrow keys and runs with Enter', () => {
    const onRun = vi.fn();
    const onClose = vi.fn();
    render(<CommandPalette entries={entries} onRun={onRun} onClose={onClose} />);
    const input = screen.getByRole('combobox');
    expect(document.activeElement).toBe(input);
    expect(screen.getAllByRole('option')).toHaveLength(3);
    expect(screen.getByText('Strg+N')).toBeTruthy();

    fireEvent.keyDown(input, { key: 'ArrowDown' });
    const active = input.getAttribute('aria-activedescendant');
    expect(active && document.getElementById(active)?.dataset['command']).toBe('focus.toggle');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onClose).toHaveBeenCalled();
    expect(onRun).toHaveBeenCalledWith('focus.toggle');
  });

  it('shows a hint when nothing matches and closes with Escape', () => {
    const onClose = vi.fn();
    render(<CommandPalette entries={entries} onRun={vi.fn()} onClose={onClose} />);
    const input = screen.getByRole('combobox');
    fireEvent.change(input, { target: { value: 'gibt es nicht' } });
    expect(screen.queryAllByRole('option')).toHaveLength(0);
    expect(screen.getByText('Kein passender Befehl.')).toBeTruthy();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('runs an entry on click', () => {
    const onRun = vi.fn();
    render(<CommandPalette entries={entries} onRun={onRun} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Vergrößern'));
    expect(onRun).toHaveBeenCalledWith('zoom.in');
  });
});
