import { describe, expect, it } from 'vitest';
import {
  chordFromKey,
  commandForChord,
  defaultKeybindings,
  effectiveKeybindings,
  findConflicts,
  formatChord,
  isCanonicalChord,
  reservedChords,
  type KeyInput,
} from './keymap';
import type { CommandId } from './types';

const none = { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false };

function key(input: Partial<KeyInput> & { key: string }): KeyInput {
  return { ...none, ...input };
}

describe('chordFromKey', () => {
  it.each([
    [key({ key: 'p', ctrlKey: true }), 'Ctrl+P'],
    [key({ key: 'P', ctrlKey: true, shiftKey: true }), 'Ctrl+Shift+P'],
    [key({ key: 'n', ctrlKey: true }), 'Ctrl+N'],
    [key({ key: '+', ctrlKey: true, shiftKey: true }), 'Ctrl+Plus'],
    [key({ key: '+', ctrlKey: true }), 'Ctrl+Plus'],
    [key({ key: '=', ctrlKey: true }), 'Ctrl+='],
    [key({ key: '-', ctrlKey: true }), 'Ctrl+-'],
    [key({ key: '0', ctrlKey: true }), 'Ctrl+0'],
    [key({ key: ',', ctrlKey: true }), 'Ctrl+,'],
    [key({ key: '?', ctrlKey: true, shiftKey: true }), 'Ctrl+?'],
    [key({ key: 'ö', altKey: true }), 'Alt+Ö'],
    [key({ key: 'F11' }), 'F11'],
    [key({ key: 'F2', shiftKey: true }), 'Shift+F2'],
    [key({ key: 'PageDown', ctrlKey: true }), 'Ctrl+PageDown'],
    [key({ key: 'Tab', ctrlKey: true, shiftKey: true }), 'Ctrl+Shift+Tab'],
    [key({ key: 'Esc', ctrlKey: true }), 'Ctrl+Escape'],
    [key({ key: ' ', ctrlKey: true }), 'Ctrl+Space'],
  ])('%o is %s', (input, chord) => {
    expect(chordFromKey(input, 'windows')).toBe(chord);
    expect(isCanonicalChord(chord)).toBe(true);
  });

  it.each([
    key({ key: 'a' }),
    key({ key: 'A', shiftKey: true }),
    key({ key: 'Enter' }),
    key({ key: 'Tab', shiftKey: true }),
    key({ key: 'Control', ctrlKey: true }),
    key({ key: 'Shift', shiftKey: true }),
    key({ key: 'Dead', ctrlKey: true }),
    key({ key: 'Unidentified', ctrlKey: true }),
    // AltGr+Q types "@" on German keyboards (Windows reports Ctrl+Alt).
    key({ key: '@', ctrlKey: true, altKey: true, altGraph: true }),
    // The Windows key belongs to the system.
    key({ key: 'e', metaKey: true }),
  ])('%o is no shortcut', (input) => {
    expect(chordFromKey(input, 'windows')).toBeNull();
  });

  it('uses Cmd instead of Ctrl on macOS', () => {
    expect(chordFromKey(key({ key: 'p', metaKey: true, shiftKey: true }), 'macos')).toBe('Ctrl+Shift+P');
    expect(chordFromKey(key({ key: 'p', ctrlKey: true }), 'macos')).toBeNull();
  });
});

describe('isCanonicalChord (same rules as Rust)', () => {
  it.each([
    'P',
    'Shift+P',
    'Ctrl+p',
    'Shift+Ctrl+P',
    'Ctrl+Ctrl+P',
    'Meta+P',
    'Ctrl+',
    'Ctrl+Shift+=',
    'Ctrl+Shift+Plus',
    'Enter',
    'Ctrl+AB',
    'F25',
    'Ctrl+ ',
  ])('rejects %s', (chord) => {
    expect(isCanonicalChord(chord)).toBe(false);
  });
});

describe('defaults', () => {
  const commands = Object.keys(defaultKeybindings) as CommandId[];
  const bindings = effectiveKeybindings(commands, {});

  it('are canonical and never reserved for text editing', () => {
    for (const chords of bindings.values()) {
      for (const chord of chords) {
        expect(isCanonicalChord(chord), chord).toBe(true);
        expect(reservedChords.has(chord), chord).toBe(false);
      }
    }
  });

  it('follow section 11', () => {
    expect(defaultKeybindings['document.new']).toEqual(['Ctrl+N']);
    expect(defaultKeybindings['document.save']).toEqual(['Ctrl+S']);
    expect(defaultKeybindings['tab.close']).toEqual(['Ctrl+W']);
    expect(defaultKeybindings['palette.show']).toEqual(['Ctrl+Shift+P']);
    expect(defaultKeybindings['focus.toggle']).toEqual(['F11']);
  });

  it('have no conflicts', () => {
    expect(findConflicts(bindings)).toEqual([]);
  });
});

describe('overrides and conflicts', () => {
  const commands: CommandId[] = ['palette.show', 'document.new', 'focus.toggle'];

  it('replace the defaults per command; an empty list unbinds', () => {
    const bindings = effectiveKeybindings(commands, { 'palette.show': ['Ctrl+K'], 'focus.toggle': [] });
    expect(bindings.get('palette.show')).toEqual(['Ctrl+K']);
    expect(bindings.get('document.new')).toEqual(['Ctrl+N']);
    expect(bindings.get('focus.toggle')).toEqual([]);
    expect(commandForChord(bindings, 'Ctrl+K')).toBe('palette.show');
    expect(commandForChord(bindings, 'Ctrl+Shift+P')).toBeNull();
    expect(commandForChord(bindings, 'F11')).toBeNull();
  });

  it('are found and make the chord inactive', () => {
    const bindings = effectiveKeybindings(commands, { 'palette.show': ['Ctrl+N'] });
    expect(findConflicts(bindings)).toEqual([
      { chord: 'Ctrl+N', commands: ['palette.show', 'document.new'] },
    ]);
    expect(commandForChord(bindings, 'Ctrl+N')).toBe('conflict');
  });
});

describe('formatChord', () => {
  it('uses the names of the keyboard labels', () => {
    expect(formatChord('Ctrl+Shift+P', 'de', 'windows')).toBe('Strg+Umschalt+P');
    expect(formatChord('Ctrl+Shift+P', 'en', 'linux')).toBe('Ctrl+Shift+P');
    expect(formatChord('Ctrl+PageDown', 'de', 'windows')).toBe('Strg+Bild ab');
    expect(formatChord('Ctrl+Plus', 'en', 'windows')).toBe('Ctrl++');
    expect(formatChord('Ctrl+Shift+P', 'de', 'macos')).toBe('⌘+⇧+P');
  });
});
