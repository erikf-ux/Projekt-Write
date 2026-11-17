import { describe, expect, it } from 'vitest';
import { createRegistry, RegistrySealedError, sealAll, type RegistryEntry } from './registry';

interface Entry extends RegistryEntry {
  value: number;
}

describe('registry', () => {
  it('keeps entries in registration order and finds them by id', () => {
    const registry = createRegistry<Entry>('test entry');
    registry.register({ id: 'b', value: 1 });
    registry.register({ id: 'a.x-2', value: 2 });
    expect(registry.list().map((entry) => entry.id)).toEqual(['b', 'a.x-2']);
    expect(registry.get('a.x-2')?.value).toBe(2);
    expect(registry.get('missing')).toBeUndefined();
    expect(registry.require('b').value).toBe(1);
    expect(registry.kind).toBe('test entry');
  });

  it('throws when a required entry is missing', () => {
    const registry = createRegistry<Entry>('test entry');
    expect(() => registry.require('missing')).toThrow('No test entry "missing" is registered');
  });

  it('refuses invalid ids', () => {
    const registry = createRegistry<Entry>('test entry');
    for (const id of ['', 'A', '1a', '-a', '.a', 'a_b', 'a b', 'ä', 'a/b']) {
      expect(() => {
        registry.register({ id, value: 0 });
      }).toThrow(/Invalid test entry id/);
    }
    expect(registry.list()).toEqual([]);
  });

  it('refuses a duplicate id', () => {
    const registry = createRegistry<Entry>('test entry');
    registry.register({ id: 'a', value: 1 });
    expect(() => {
      registry.register({ id: 'a', value: 2 });
    }).toThrow('The test entry "a" is registered twice');
    expect(registry.require('a').value).toBe(1);
  });

  it('takes nothing after seal()', () => {
    const registry = createRegistry<Entry>('test entry');
    registry.register({ id: 'a', value: 1 });
    expect(registry.sealed).toBe(false);
    registry.seal();
    registry.seal();
    expect(registry.sealed).toBe(true);
    expect(() => {
      registry.register({ id: 'b', value: 2 });
    }).toThrow(RegistrySealedError);
    expect(registry.list().map((entry) => entry.id)).toEqual(['a']);
    expect(registry.require('a').value).toBe(1);
  });

  it('cannot be changed behind its back', () => {
    const registry = createRegistry<Entry>('test entry');
    const entry = { id: 'a', value: 1 };
    registry.register(entry);
    registry.seal();
    expect(() => {
      entry.value = 2;
    }).toThrow(TypeError);
    expect(() => {
      (registry.list() as Entry[]).push({ id: 'b', value: 2 });
    }).toThrow(TypeError);
    expect(() => {
      (registry as { seal: () => void }).seal = () => undefined;
    }).toThrow(TypeError);
    expect(registry.list().map((item) => item.id)).toEqual(['a']);
  });

  it('seals several registries at once', () => {
    const first = createRegistry<Entry>('first');
    const second = createRegistry<Entry>('second');
    sealAll([first, second]);
    expect(first.sealed && second.sealed).toBe(true);
    expect(() => {
      second.register({ id: 'a', value: 1 });
    }).toThrow('The second registry is sealed: "a" cannot be registered after start');
  });
});
