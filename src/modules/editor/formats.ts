/**
 * Document formats on the editor side (ADR 0027, decision 2): how the
 * writing surface reads and writes `txt`, `md` and `pwrite`. The session
 * takes the adapter of its format from the registry.
 */
import type { JSONContent } from '@tiptap/core';
import type { RegistryEntry } from '@/core/registry';
import { registries } from '@/extensions/registries';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import { serializeMarkdown } from './markdown/adapter';
import { markdownSchema, probeMarkdown, type ProbeIssue } from './markdown/probe';

/** A text as the editor shows it after opening. */
export type OpenedText =
  /** Rich text in the Markdown schema; `frontMatter` is kept aside (Markdown only). */
  | { view: 'rich'; content: JSONContent; frontMatter: string; issues: ProbeIssue[] }
  /** Text in the minimal schema; `issues` say why it is not rich text. */
  | { view: 'source'; text: string; issues: ProbeIssue[] };

export interface DocumentFormatAdapter extends RegistryEntry {
  readonly id: DocumentFormat;
  /**
   * Rich text that can also be shown as its source (Markdown): the view
   * can switch, the source view has no language check, and saving the
   * rich view runs the loss check first.
   */
  readonly sourceView: boolean;
  /** Reads a text as the format holds it. */
  readonly open: (text: string) => OpenedText;
  /** Writes rich content (unused by formats that never show rich text). */
  readonly serialize: (content: JSONContent, frontMatter: string) => string;
}

/** An empty `.pwrite` document (one empty paragraph). */
const EMPTY_PWRITE: JSONContent = { type: 'doc', content: [{ type: 'paragraph' }] };

/**
 * The content of a `.pwrite` document as Rust hands it over (JSON of the
 * editor document), or null if it does not fit the schema.
 */
export function pwriteContent(text: string): JSONContent | null {
  try {
    const content = JSON.parse(text) as JSONContent;
    markdownSchema().nodeFromJSON(content).check();
    return content;
  } catch {
    return null;
  }
}

/** `.md`: rich text if the round-trip probe finds nothing, else the source view. */
export const markdownFormat: DocumentFormatAdapter = {
  id: 'md',
  sourceView: true,
  open: (text) => {
    const probe = probeMarkdown(text);
    return probe.issues.length === 0
      ? { view: 'rich', content: probe.document.body, frontMatter: probe.document.frontMatter, issues: [] }
      : { view: 'source', text, issues: probe.issues };
  },
  serialize: (content, frontMatter) => serializeMarkdown({ frontMatter, body: content }),
};

/** `.pwrite` holds what the editor holds: no probe, no source view. */
export const pwriteFormat: DocumentFormatAdapter = {
  id: 'pwrite',
  sourceView: false,
  open: (text) => {
    const content = pwriteContent(text);
    return {
      view: 'rich',
      content: content ?? EMPTY_PWRITE,
      frontMatter: '',
      issues: content ? [] : [{ kind: 'invalid' }],
    };
  },
  serialize: (content) => JSON.stringify(content),
};

/** `.txt`: the minimal schema only. */
export const plainTextFormat: DocumentFormatAdapter = {
  id: 'txt',
  sourceView: false,
  open: (text) => ({ view: 'source', text, issues: [] }),
  serialize: () => {
    throw new Error('plain text has no rich view');
  },
};

/** The adapter of a format. */
export function formatAdapter(format: DocumentFormat): DocumentFormatAdapter {
  return registries.documentFormats.require(format);
}
