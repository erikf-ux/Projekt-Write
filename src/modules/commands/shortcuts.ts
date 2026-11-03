/**
 * The one place where shortcuts are handled: a key listener on the window
 * (capture phase), so shortcuts work wherever the focus is.
 */
import type { Platform } from '@/types/generated/Platform';
import { chordFromKey, commandForChord, suppressedBrowserChords } from './keymap';
import { runCommand } from './registry';
import type { CommandId } from './types';

export interface ShortcutOptions {
  platform: () => Platform;
  bindings: () => ReadonlyMap<CommandId, readonly string[]>;
  /** A dialog records a new shortcut: keys go to it, not to commands. */
  isRecording: () => boolean;
  onConflict: (chord: string) => void;
}

export function handleKeyDown(event: KeyboardEvent, options: ShortcutOptions): void {
  if (event.defaultPrevented || event.isComposing || options.isRecording()) {
    return;
  }
  const chord = chordFromKey(
    {
      key: event.key,
      ctrlKey: event.ctrlKey,
      altKey: event.altKey,
      shiftKey: event.shiftKey,
      metaKey: event.metaKey,
      altGraph: event.getModifierState('AltGraph'),
    },
    options.platform(),
  );
  if (chord === null) {
    return;
  }
  const command = commandForChord(options.bindings(), chord);
  if (command === 'conflict') {
    event.preventDefault();
    options.onConflict(chord);
    return;
  }
  if (command !== null) {
    event.preventDefault();
    event.stopPropagation();
    void runCommand(command);
    return;
  }
  if (suppressedBrowserChords.has(chord)) {
    event.preventDefault();
  }
}

/** Installs the listener; returns the function that removes it. */
export function installShortcuts(options: ShortcutOptions, target: Window = window): () => void {
  const listener = (event: KeyboardEvent) => {
    handleKeyDown(event, options);
  };
  target.addEventListener('keydown', listener, { capture: true });
  return () => {
    target.removeEventListener('keydown', listener, { capture: true });
  };
}
