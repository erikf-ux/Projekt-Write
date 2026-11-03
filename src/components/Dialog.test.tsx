import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocale } from '@/i18n';
import { Dialog } from './Dialog';

describe('Dialog', () => {
  beforeEach(() => {
    useLocale.getState().setLocale('de');
  });

  it('closes only the innermost dialog with Escape, wherever the focus is', () => {
    const outer = vi.fn();
    const inner = vi.fn();
    render(
      <Dialog title="Außen" onClose={outer}>
        <button type="button">A</button>
        <Dialog title="Innen" onClose={inner}>
          <button type="button">B</button>
        </Dialog>
      </Dialog>,
    );
    (document.activeElement as HTMLElement | null)?.blur();
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(inner).toHaveBeenCalledTimes(1);
    expect(outer).not.toHaveBeenCalled();
  });

  it('keeps the focus inside and returns it afterwards', () => {
    const trigger = document.createElement('button');
    document.body.append(trigger);
    trigger.focus();
    const { unmount } = render(
      <Dialog title="Test" onClose={vi.fn()}>
        <button type="button">Erster</button>
        <button type="button">Letzter</button>
      </Dialog>,
    );
    const close = screen.getByRole('button', { name: 'Schließen' });
    expect(document.activeElement).toBe(close);
    trigger.focus();
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
});
