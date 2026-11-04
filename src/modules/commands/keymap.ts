/**
 * Shortcuts (section 11: "Konfigurierbare Tastenkürzel mit Konflikterkennung").
 *
 * A chord has one canonical form, the same Rust validates
 * (`services/settings/ui.rs`): modifiers `Ctrl`, `Alt`, `Shift` in this
 * order, then one key. On macOS `Ctrl` stands for `Cmd`.
 *
 * * Letters are upper case; `Shift` counts only with letters and named
 *   keys, because for other characters the character already reflects it
 *   (`Ctrl+?`, not `Ctrl+Shift+ß`).
 * * Every key except F1–F24 needs `Ctrl` or `Alt`, so shortcuts never
 *   swallow typing.
 * * Chords for text editing (copy, paste, undo, word navigation) are
 *   reserved and cannot be assigned.
 */
import type { Locale } from '@/i18n';
import type { Platform } from '@/types/generated/Platform';
import type { CommandId } from './types';

/** Shortcuts of section 11 for the commands of Phase 3, and a few more. */
export const defaultKeybindings: Readonly<Partial<Record<CommandId, readonly string[]>>> = {
  'document.new': ['Ctrl+N'],
  'document.save': ['Ctrl+S'],
  'document.rename': ['F2'],
  'tab.close': ['Ctrl+W'],
  'tab.next': ['Ctrl+PageDown', 'Ctrl+Tab'],
  'tab.previous': ['Ctrl+PageUp', 'Ctrl+Shift+Tab'],
  'tab.moveLeft': ['Ctrl+Shift+PageUp'],
  'tab.moveRight': ['Ctrl+Shift+PageDown'],
  'palette.show': ['Ctrl+Shift+P'],
  'settings.open': ['Ctrl+,'],
  'app.quit': ['Ctrl+Q'],
  'sidebar.toggle': ['Ctrl+Shift+B'],
  'explorer.focus': ['Ctrl+Shift+E'],
  'focus.toggle': ['F11'],
  'editor.toggleSource': ['Ctrl+/'],
  'find.show': ['Ctrl+F'],
  'replace.show': ['Ctrl+H'],
  'find.next': ['F3'],
  'find.previous': ['Shift+F3'],
  'format.bold': ['Ctrl+B'],
  'format.italic': ['Ctrl+I'],
  'format.underline': ['Ctrl+U'],
  'format.code': ['Ctrl+E'],
  'format.link': ['Ctrl+K'],
  'zoom.in': ['Ctrl+Plus', 'Ctrl+='],
  'zoom.out': ['Ctrl+-'],
  'zoom.reset': ['Ctrl+0'],
};

/** Text editing in the writing surface; never assignable. */
export const reservedChords: ReadonlySet<string> = new Set([
  'Ctrl+A',
  'Ctrl+C',
  'Ctrl+V',
  'Ctrl+X',
  'Ctrl+Z',
  'Ctrl+Y',
  'Ctrl+Shift+Z',
  'Ctrl+Shift+V',
  'Ctrl+Backspace',
  'Ctrl+Delete',
  'Ctrl+ArrowLeft',
  'Ctrl+ArrowRight',
  'Ctrl+ArrowUp',
  'Ctrl+ArrowDown',
  'Ctrl+Shift+ArrowLeft',
  'Ctrl+Shift+ArrowRight',
  'Ctrl+Shift+ArrowUp',
  'Ctrl+Shift+ArrowDown',
  'Ctrl+Home',
  'Ctrl+End',
  'Ctrl+Shift+Home',
  'Ctrl+Shift+End',
  'Alt+F4',
]);

/**
 * Browser functions of the webview that must not run inside the app (reload
 * would discard what is not saved yet, print and find come with later
 * phases). Their default action is always prevented.
 */
export const suppressedBrowserChords: ReadonlySet<string> = new Set([
  'F5',
  'Ctrl+F5',
  'Ctrl+R',
  'Ctrl+Shift+R',
  'Ctrl+P',
  'Ctrl+F',
  'Ctrl+G',
  'Ctrl+Shift+G',
  'Ctrl+H',
  'Ctrl+U',
  'Ctrl+J',
  'Ctrl+Shift+I',
  'Ctrl+Shift+J',
  'Ctrl+Shift+C',
  'F3',
  'F7',
  'F12',
  'Alt+ArrowLeft',
  'Alt+ArrowRight',
  'Alt+Home',
]);

const NAMED_KEYS = new Set([
  'Space',
  'Enter',
  'Escape',
  'Tab',
  'Backspace',
  'Delete',
  'Insert',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
]);

const LEGACY_KEY_NAMES: Record<string, string> = {
  ' ': 'Space',
  Spacebar: 'Space',
  Esc: 'Escape',
  Up: 'ArrowUp',
  Down: 'ArrowDown',
  Left: 'ArrowLeft',
  Right: 'ArrowRight',
  Del: 'Delete',
  '+': 'Plus',
  Add: 'Plus',
};

function isFunctionKey(key: string): boolean {
  return /^F([1-9]|1[0-9]|2[0-4])$/.test(key);
}

/** Whether `text` is exactly one Unicode code point. */
function isOneCodePoint(text: string): boolean {
  const first = text.codePointAt(0);
  return first !== undefined && text.length === (first > 0xffff ? 2 : 1);
}

function hasCase(character: string): boolean {
  return character.toLowerCase() !== character.toUpperCase();
}

/** The parts of a keyboard event a chord depends on. */
export interface KeyInput {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  /** `AltGr` held (typing a character such as `@` on German keyboards). */
  altGraph?: boolean;
}

/** The canonical chord of a key press, or `null` if it is no shortcut. */
export function chordFromKey(input: KeyInput, platform: Platform): string | null {
  const ctrl = platform === 'macos' ? input.metaKey : input.ctrlKey;
  if (platform !== 'macos' && input.metaKey) {
    return null;
  }
  const mapped = LEGACY_KEY_NAMES[input.key] ?? input.key;
  let key: string;
  if (isFunctionKey(mapped) || NAMED_KEYS.has(mapped) || mapped === 'Plus') {
    key = mapped;
  } else if (isOneCodePoint(mapped) && !/\s/.test(mapped)) {
    const upper = mapped.toUpperCase();
    key = isOneCodePoint(upper) ? upper : mapped;
  } else {
    // Modifier keys alone, dead keys, media keys and the like.
    return null;
  }
  if (input.altGraph && !isFunctionKey(key)) {
    return null;
  }
  // `+` is a character too: Shift is part of typing it on many layouts.
  const character = isOneCodePoint(key) || key === 'Plus';
  const shift = input.shiftKey && (!character || (key !== 'Plus' && hasCase(key)));
  const parts: string[] = [];
  if (ctrl) parts.push('Ctrl');
  if (input.altKey) parts.push('Alt');
  if (shift) parts.push('Shift');
  if (!isFunctionKey(key) && !ctrl && !input.altKey) {
    return null;
  }
  parts.push(key);
  return parts.join('+');
}

/** Same rules as `validate_chord` in Rust, for chords typed by the user. */
export function isCanonicalChord(chord: string): boolean {
  const parts = chord.split('+');
  const key = parts.pop();
  if (key === undefined || key === '') {
    return false;
  }
  const order = ['Ctrl', 'Alt', 'Shift'];
  let previous = -1;
  for (const modifier of parts) {
    const index = order.indexOf(modifier);
    if (index <= previous) {
      return false;
    }
    previous = index;
  }
  const command = parts.includes('Ctrl') || parts.includes('Alt');
  if (isFunctionKey(key)) {
    return true;
  }
  if (key === 'Plus') {
    return command && !parts.includes('Shift');
  }
  if (NAMED_KEYS.has(key)) {
    return command;
  }
  if (!isOneCodePoint(key) || /[\s+]/.test(key) || /\p{Cc}/u.test(key)) {
    return false;
  }
  if (key !== key.toUpperCase() && isOneCodePoint(key.toUpperCase())) {
    return false;
  }
  if (parts.includes('Shift') && !hasCase(key)) {
    return false;
  }
  return command;
}

export type Keybindings = Partial<Record<string, readonly string[]>>;

/** The chords of every command: the user's overrides, else the defaults. */
export function effectiveKeybindings(
  commands: readonly CommandId[],
  overrides: Keybindings,
): Map<CommandId, readonly string[]> {
  const result = new Map<CommandId, readonly string[]>();
  for (const id of commands) {
    result.set(id, overrides[id] ?? defaultKeybindings[id] ?? []);
  }
  return result;
}

export interface Conflict {
  chord: string;
  commands: CommandId[];
}

/** Chords assigned to more than one command. */
export function findConflicts(bindings: ReadonlyMap<CommandId, readonly string[]>): Conflict[] {
  const byChord = new Map<string, CommandId[]>();
  for (const [id, chords] of bindings) {
    for (const chord of chords) {
      byChord.set(chord, [...(byChord.get(chord) ?? []), id]);
    }
  }
  return [...byChord]
    .filter(([, ids]) => ids.length > 1)
    .map(([chord, ids]) => ({ chord, commands: ids }))
    .sort((a, b) => a.chord.localeCompare(b.chord));
}

/**
 * The command a chord runs. A chord with a conflict runs nothing until the
 * conflict is resolved (it is reported instead).
 */
export function commandForChord(
  bindings: ReadonlyMap<CommandId, readonly string[]>,
  chord: string,
): CommandId | 'conflict' | null {
  const matches = [...bindings].filter(([, chords]) => chords.includes(chord)).map(([id]) => id);
  if (matches.length > 1) {
    return 'conflict';
  }
  return matches[0] ?? null;
}

const GERMAN_KEY_NAMES: Record<string, string> = {
  Ctrl: 'Strg',
  Shift: 'Umschalt',
  PageUp: 'Bild auf',
  PageDown: 'Bild ab',
  Escape: 'Esc',
  Delete: 'Entf',
  Insert: 'Einfg',
  Home: 'Pos1',
  End: 'Ende',
  Enter: 'Eingabe',
  Space: 'Leertaste',
  Backspace: 'Rücktaste',
};

const SHARED_KEY_NAMES: Record<string, string> = {
  Plus: '+',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
};

const MAC_KEY_NAMES: Record<string, string> = { Ctrl: '⌘', Alt: '⌥', Shift: '⇧' };

/** A chord as shown to the user: `Strg+Umschalt+P`, `Ctrl+Shift+P`, `⌘+⇧+P`. */
export function formatChord(chord: string, locale: Locale, platform: Platform): string {
  return chord
    .split('+')
    .map((part) => {
      if (platform === 'macos' && MAC_KEY_NAMES[part]) {
        return MAC_KEY_NAMES[part];
      }
      return SHARED_KEY_NAMES[part] ?? (locale === 'de' ? GERMAN_KEY_NAMES[part] : undefined) ?? part;
    })
    .join('+');
}
