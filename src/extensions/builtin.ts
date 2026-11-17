/**
 * Every built-in extension of the app, in one place (ADR 0027, decision 2).
 * `main.tsx` calls `registerBuiltins()` before the first render and then
 * seals the registries; nothing can be added after that (decision 3).
 *
 * The order of the editor features is the order of the Tiptap extensions:
 * priorities, keyboard shortcuts and the Markdown tokenizers depend on it
 * (`features.test.ts` compares the lists with those before the registries).
 */
import FindAndReplace from '@tiptap/extension-find-and-replace';
import { lazy } from 'react';
import type { EditorFeature } from '@/modules/editor/features';
import { markdownFormat, plainTextFormat, pwriteFormat } from '@/modules/editor/formats';
import { plainExtensions } from '@/modules/editor/plain/schema';
import {
  coreTextExtensions,
  entityExtensions,
  imageExtensions,
  languagePartExtensions,
  linkExtensions,
  listExtensions,
  rawExtensions,
  tableExtensions,
  taskListExtensions,
} from '@/modules/editor/schema/extensions';
import {
  libraryExporter,
  markdownExporter,
  packExporter,
  pdfExporter,
  printExporter,
  txtExporter,
} from '@/modules/export/exporters';
import { LanguageCheck } from '@/modules/language-check/plugin';
import { MESH_RENDERER } from '@/modules/mesh/renderer';
import { registries as appRegistries, type AppRegistries } from './registries';

/** Groups of the Markdown schema (also the schema of `.pwrite` rich text). */
const markdownFeature = (id: string, extensions: EditorFeature['extensions']): EditorFeature => ({
  id,
  schemas: ['markdown'],
  editorOnly: false,
  extensions,
});

const EDITOR_FEATURES: readonly EditorFeature[] = [
  markdownFeature('core-text', coreTextExtensions),
  markdownFeature('lists', listExtensions),
  markdownFeature('links', linkExtensions),
  markdownFeature('entities', entityExtensions),
  markdownFeature('task-lists', taskListExtensions),
  markdownFeature('tables', tableExtensions),
  markdownFeature('images', imageExtensions),
  markdownFeature('raw-markdown', rawExtensions),
  markdownFeature('language-parts', languagePartExtensions),
  {
    id: 'plain-text',
    schemas: ['plain'],
    editorOnly: false,
    extensions: plainExtensions,
  },
  {
    // Find and replace (section 12); highlights styled in `editor.css`
    // (no inline style, CSP).
    id: 'find',
    schemas: ['markdown', 'plain'],
    editorOnly: true,
    extensions: () => [FindAndReplace.configure({ injectCSS: false, searchDebounceMs: 0 })],
  },
  {
    // The language check (section 14). The Markdown source view is syntax,
    // not text: no language check there.
    id: 'language-check',
    schemas: ['markdown', 'plain'],
    editorOnly: true,
    extensions: ({ documentId, source }) => [LanguageCheck.configure({ documentId, enabled: !source })],
  },
];

/** Fills the registries with the built-in entries (once, at start). */
export function registerBuiltins(target: AppRegistries = appRegistries): void {
  for (const feature of EDITOR_FEATURES) {
    target.editorFeatures.register(feature);
  }

  target.documentFormats.register(plainTextFormat);
  target.documentFormats.register(markdownFormat);
  target.documentFormats.register(pwriteFormat);

  // The order is the order of the commands in the palette and the
  // shortcut settings.
  target.exporters.register(printExporter);
  target.exporters.register(pdfExporter);
  target.exporters.register(txtExporter);
  target.exporters.register(markdownExporter);
  target.exporters.register(packExporter);
  target.exporters.register(libraryExporter);

  target.meshRenderers.register({
    id: MESH_RENDERER,
    // React Flow is large: loaded when the Mesh is first shown.
    component: lazy(() => import('@/modules/mesh/MeshCanvas')),
  });
}
