/**
 * Converting a document between `.txt`, `.md` and `.pwrite` (section 10;
 * ADR 0015, decision 13). The editor converts, Rust stores the result.
 *
 * * TXT becomes paragraphs: blank lines separate them, single line breaks
 *   stay as line breaks.
 * * MD and `.pwrite` go over into each other through the editor document;
 *   to MD, `findLosses` names what Markdown cannot hold (ADR 0014). The
 *   YAML front matter of a Markdown file has no place in `.pwrite`.
 * * MD to TXT keeps the Markdown text as it is.
 * * `.pwrite` to TXT keeps the text and loses every formatting.
 */
import type { JSONContent } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import type { DocumentFormat } from '@/types/generated/DocumentFormat';
import { serializeMarkdown } from './markdown/adapter';
import { findLosses, type Loss } from './markdown/losses';
import { markdownSchema, probeMarkdown, type ProbeIssue } from './markdown/probe';

/** What the editor of a tab holds. */
export type ConversionSource =
  { kind: 'rich'; content: JSONContent; frontMatter: string } | { kind: 'text'; text: string };

export interface Conversion {
  /** The content in the new format (`.pwrite`: the editor document as JSON). */
  text: string;
  /** What Markdown cannot hold. */
  losses: Loss[];
  /** Rich text becomes plain text. */
  formattingLost: boolean;
  /** The YAML front matter does not go into `.pwrite`. */
  frontMatterLost: boolean;
  /** Markdown the editor does not know; it may change. */
  issues: ProbeIssue[];
}

/** Paragraphs from plain text: blank lines separate them, line feeds become line breaks. */
export function paragraphsFromText(text: string): JSONContent {
  const paragraphs = text
    .replace(/\r\n?/g, '\n')
    .split(/\n[ \t]*\n+/)
    .filter((block) => block.trim() !== '')
    .map((block): JSONContent => {
      const content: JSONContent[] = [];
      block.split('\n').forEach((line, index) => {
        if (index > 0) {
          content.push({ type: 'hardBreak' });
        }
        if (line !== '') {
          content.push({ type: 'text', text: line });
        }
      });
      return { type: 'paragraph', content };
    });
  return { type: 'doc', content: paragraphs.length > 0 ? paragraphs : [{ type: 'paragraph' }] };
}

/** The text of an editor document: blocks separated by a blank line. */
export function plainTextOf(content: JSONContent): string {
  const node: ProseMirrorNode = markdownSchema().nodeFromJSON(content);
  return node.textBetween(0, node.content.size, '\n\n', (leaf) =>
    leaf.type.name === 'hardBreak' ? '\n' : '',
  );
}

const NOTHING_LOST = { losses: [], formattingLost: false, frontMatterLost: false, issues: [] };

/** The editor document and front matter of the source. */
function richOf(
  from: DocumentFormat,
  source: ConversionSource,
): {
  content: JSONContent;
  frontMatter: string;
  issues: ProbeIssue[];
} {
  if (source.kind === 'rich') {
    return { content: source.content, frontMatter: source.frontMatter, issues: [] };
  }
  if (from === 'txt') {
    return { content: paragraphsFromText(source.text), frontMatter: '', issues: [] };
  }
  if (from === 'pwrite') {
    return { content: JSON.parse(source.text) as JSONContent, frontMatter: '', issues: [] };
  }
  // Markdown in the source view: the editor reads it as it would open it.
  const probe = probeMarkdown(source.text);
  return { content: probe.document.body, frontMatter: probe.document.frontMatter, issues: probe.issues };
}

/** Converts the content of a tab from `from` to `to` (different formats). */
export function convertContent(
  from: DocumentFormat,
  to: DocumentFormat,
  source: ConversionSource,
): Conversion {
  if (to === 'txt') {
    if (from === 'md') {
      const text =
        source.kind === 'text'
          ? source.text
          : serializeMarkdown({ frontMatter: source.frontMatter, body: source.content });
      return { text, ...NOTHING_LOST };
    }
    const { content } = richOf(from, source);
    return { text: plainTextOf(content), ...NOTHING_LOST, formattingLost: true };
  }
  const { content, frontMatter, issues } = richOf(from, source);
  if (to === 'md') {
    return {
      text: serializeMarkdown({ frontMatter, body: content }),
      ...NOTHING_LOST,
      losses: findLosses(content),
      issues,
    };
  }
  return {
    text: JSON.stringify(content),
    ...NOTHING_LOST,
    frontMatterLost: frontMatter !== '',
    issues,
  };
}

/** Whether the user must confirm before converting. */
export function needsConfirmation(conversion: Conversion): boolean {
  return (
    conversion.losses.length > 0 ||
    conversion.formattingLost ||
    conversion.frontMatterLost ||
    conversion.issues.length > 0
  );
}
