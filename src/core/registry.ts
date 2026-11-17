/**
 * Internal registries (section 22, ADR 0027, decision 2): editor features,
 * document formats, exporters and Mesh renderers are looked up by id
 * instead of being wired into the modules that use them.
 *
 * A registry is filled once at start (`src/extensions/builtin.ts`) and then
 * sealed; after that it takes nothing more (ADR 0027, decision 3: no
 * foreign code). The registries live in module scope and are never put on
 * `window`.
 */

/** Ids are lower case: a letter, then letters, digits, `.` and `-`. */
export const REGISTRY_ID_PATTERN = /^[a-z][a-z0-9.-]*$/;

export interface RegistryEntry {
  readonly id: string;
}

/** `register` after `seal()`: the registry takes nothing after start. */
export class RegistrySealedError extends Error {
  readonly kind: string;
  readonly entryId: string;

  constructor(kind: string, entryId: string) {
    super(`The ${kind} registry is sealed: "${entryId}" cannot be registered after start`);
    this.name = 'RegistrySealedError';
    this.kind = kind;
    this.entryId = entryId;
  }
}

export interface Registry<T extends RegistryEntry> {
  /** What the registry holds, for messages ("exporter", "mesh renderer"). */
  readonly kind: string;
  /** Whether `seal()` was called. */
  readonly sealed: boolean;
  /**
   * Adds an entry. Throws on an invalid or duplicate id (a programming
   * error) and, after `seal()`, a {@link RegistrySealedError}.
   */
  register(entry: T): void;
  get(id: string): T | undefined;
  /** The entry with this id; throws if there is none. */
  require(id: string): T;
  /** All entries in registration order. */
  list(): readonly T[];
  /** Closes the registry for good. Calling it again changes nothing. */
  seal(): void;
}

export function createRegistry<T extends RegistryEntry>(kind: string): Registry<T> {
  const entries = new Map<string, T>();
  let sealed = false;
  const registry: Registry<T> = {
    kind,
    get sealed() {
      return sealed;
    },
    register(entry: T): void {
      if (sealed) {
        throw new RegistrySealedError(kind, entry.id);
      }
      if (!REGISTRY_ID_PATTERN.test(entry.id)) {
        throw new Error(`Invalid ${kind} id "${entry.id}": expected ${String(REGISTRY_ID_PATTERN)}`);
      }
      if (entries.has(entry.id)) {
        throw new Error(`The ${kind} "${entry.id}" is registered twice`);
      }
      // The entry cannot be swapped out from under the registry later.
      entries.set(entry.id, Object.freeze(entry));
    },
    get(id: string): T | undefined {
      return entries.get(id);
    },
    require(id: string): T {
      const entry = entries.get(id);
      if (entry === undefined) {
        throw new Error(`No ${kind} "${id}" is registered`);
      }
      return entry;
    },
    list(): readonly T[] {
      return Object.freeze([...entries.values()]);
    },
    seal(): void {
      sealed = true;
    },
  };
  return Object.freeze(registry);
}

/** Seals every registry in the list. */
export function sealAll(registries: Iterable<Registry<RegistryEntry>>): void {
  for (const registry of registries) {
    registry.seal();
  }
}
