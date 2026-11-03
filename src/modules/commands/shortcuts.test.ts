import { afterEach, describe, expect, it, vi } from 'vitest';
import { effectiveKeybindings } from './keymap';
import { registerCommands } from './registry';
import { installShortcuts } from './shortcuts';
import type { CommandDefinition, CommandId } from './types';

function command(id: CommandId, run: () => void, isAvailable = () => true): CommandDefinition {
  return { id, category: 'view', titleKey: 'command.focus.toggle', isAvailable, run };
}

function press(init: KeyboardEventInit): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  document.body.dispatchEvent(event);
  return event;
}

describe('installShortcuts', () => {
  let remove: () => void = () => undefined;
  afterEach(() => {
    remove();
    registerCommands([]);
  });

  function install(overrides: Record<string, string[]> = {}, onConflict = vi.fn()) {
    const ids: CommandId[] = ['focus.toggle', 'document.new', 'palette.show'];
    remove = installShortcuts({
      platform: () => 'windows',
      bindings: () => effectiveKeybindings(ids, overrides),
      isRecording: () => false,
      onConflict,
    });
    return onConflict;
  }

  it('runs the bound command and prevents the browser default', () => {
    const focus = vi.fn();
    registerCommands([command('focus.toggle', focus)]);
    install();
    const event = press({ key: 'F11' });
    expect(focus).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it('does not run unavailable commands', () => {
    const create = vi.fn();
    registerCommands([command('document.new', create, () => false)]);
    install();
    const event = press({ key: 'n', ctrlKey: true });
    expect(create).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it('reports conflicts instead of running one of the commands', () => {
    const create = vi.fn();
    const palette = vi.fn();
    registerCommands([command('document.new', create), command('palette.show', palette)]);
    const onConflict = install({ 'palette.show': ['Ctrl+N'] });
    press({ key: 'n', ctrlKey: true });
    expect(onConflict).toHaveBeenCalledWith('Ctrl+N');
    expect(create).not.toHaveBeenCalled();
    expect(palette).not.toHaveBeenCalled();
  });

  it('blocks reload and print of the webview but leaves typing alone', () => {
    registerCommands([]);
    install();
    expect(press({ key: 'F5' }).defaultPrevented).toBe(true);
    expect(press({ key: 'r', ctrlKey: true }).defaultPrevented).toBe(true);
    expect(press({ key: 'p', ctrlKey: true }).defaultPrevented).toBe(true);
    expect(press({ key: 'a' }).defaultPrevented).toBe(false);
    expect(press({ key: 'c', ctrlKey: true }).defaultPrevented).toBe(false);
  });
});
