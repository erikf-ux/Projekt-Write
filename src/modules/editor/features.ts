/**
 * Editor features (ADR 0027, decision 2): groups of Tiptap extensions per
 * schema, looked up in the registry. The order of registration is the
 * order of the extensions: Tiptap resolves priorities and the Markdown
 * tokenizers by it, so `src/extensions/builtin.ts` keeps it.
 */
import type { AnyExtension } from '@tiptap/core';
import type { RegistryEntry } from '@/core/registry';
import { registries } from '@/extensions/registries';

/**
 * `markdown`: rich text, for `.md` and `.pwrite` (one schema, so saving
 * never has to drop formatting); `plain`: the minimal schema of plain text
 * and the Markdown source view.
 */
export type EditorSchema = 'markdown' | 'plain';

/** What an editor tells its features. */
export interface EditorFeatureContext {
  /** Library id of the document; null for external files and for the bare schema. */
  documentId: string | null;
  /** The view shows Markdown syntax (source view), not text. */
  source: boolean;
}

export interface EditorFeature extends RegistryEntry {
  readonly schemas: readonly EditorSchema[];
  /**
   * Only in editors (plugins without nodes or marks, such as find): not
   * part of the schema that reads and writes documents.
   */
  readonly editorOnly: boolean;
  readonly extensions: (context: EditorFeatureContext) => AnyExtension[];
}

/** The context of the bare schema (reading and writing, no editor). */
const SCHEMA_CONTEXT: EditorFeatureContext = { documentId: null, source: false };

function features(schema: EditorSchema, withEditorOnly: boolean): EditorFeature[] {
  return registries.editorFeatures
    .list()
    .filter((feature) => feature.schemas.includes(schema) && (withEditorOnly || !feature.editorOnly));
}

/** The extensions of a schema, for parsing, serialising and checking. */
export function schemaExtensions(schema: EditorSchema): AnyExtension[] {
  return features(schema, false).flatMap((feature) => feature.extensions(SCHEMA_CONTEXT));
}

/** The extensions of an editor: the schema and the editor-only features. */
export function editorExtensions(schema: EditorSchema, context: EditorFeatureContext): AnyExtension[] {
  return features(schema, true).flatMap((feature) => feature.extensions(context));
}
