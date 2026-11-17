/**
 * The registries of the app (ADR 0027, decision 2). `builtin.ts` fills them
 * and `main.tsx` seals them before the first render; after that they take
 * nothing more (decision 3).
 *
 * They are module state, never put on `window`: code outside the bundle
 * cannot reach them. Tests that need registries of their own use
 * `createAppRegistries()`.
 */
import { createRegistry, sealAll, type Registry, type RegistryEntry } from '@/core/registry';
import type { EditorFeature } from '@/modules/editor/features';
import type { DocumentFormatAdapter } from '@/modules/editor/formats';
import type { Exporter } from '@/modules/export/exporters';
import type { MeshRenderer } from '@/modules/mesh/renderer';

export interface AppRegistries {
  readonly editorFeatures: Registry<EditorFeature>;
  readonly documentFormats: Registry<DocumentFormatAdapter>;
  readonly exporters: Registry<Exporter>;
  readonly meshRenderers: Registry<MeshRenderer>;
}

export function createAppRegistries(): AppRegistries {
  return Object.freeze({
    editorFeatures: createRegistry<EditorFeature>('editor feature'),
    documentFormats: createRegistry<DocumentFormatAdapter>('document format'),
    exporters: createRegistry<Exporter>('exporter'),
    meshRenderers: createRegistry<MeshRenderer>('mesh renderer'),
  });
}

/** The registries the app uses. */
export const registries: AppRegistries = createAppRegistries();

/** Every registry of a set, for sealing and listing. */
export function registryList(target: AppRegistries): Registry<RegistryEntry>[] {
  return [target.editorFeatures, target.documentFormats, target.exporters, target.meshRenderers];
}

/** Seals all registries: from now on `register` throws. */
export function sealRegistries(target: AppRegistries = registries): void {
  sealAll(registryList(target));
}
