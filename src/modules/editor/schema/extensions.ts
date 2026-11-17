/**
 * The schema of Markdown documents (section 12): everything Markdown can
 * hold, nothing more, so saving never has to drop formatting. Raw HTML and
 * footnotes are kept as atoms (`raw.ts`), images as notes (`image.ts`).
 */
import { LanguagePart } from './lang';
import type { AnyExtension } from '@tiptap/core';
import { TableKit } from '@tiptap/extension-table';
import StarterKit from '@tiptap/starter-kit';
import { Image } from './image';
import {
  Entities,
  LinkedImages,
  MarkdownBulletList,
  MarkdownCode,
  MarkdownCodeBlock,
  MarkdownDocument,
  MarkdownHardBreak,
  MarkdownHeading,
  MarkdownLink,
  MarkdownListItem,
  MarkdownOrderedList,
  MarkdownParagraph,
  MarkdownTable,
  MarkdownTaskItem,
  MarkdownTaskList,
} from './markdownRules';
import { Footnotes, RawBlock, RawInline } from './raw';
import { schemaExtensions } from '../features';

/** Web links the app may open (section 12). */
export const LINK_PROTOCOLS = ['http', 'https', 'mailto'] as const;

/** Internal links point to a document id (ADR 0015, decision 14). */
export const INTERNAL_LINK_PREFIX = 'pwrite://';

/** The document id of an internal link, or null. */
export function internalLinkTarget(href: string): string | null {
  const match = /^pwrite:\/\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/.exec(
    href.trim(),
  );
  return match?.[1] ?? null;
}

/**
 * Text that becomes a link by itself, while typing or pasting: addresses
 * with `http://`, `https://`, `mailto:` or `www.`, and e-mail addresses.
 * Not bare domains: in prose a missing space after a full stop ("im.Am",
 * "geht.so") would become a link to a country domain.
 */
export function isWebAddress(text: string): boolean {
  return /^(?:https?:\/\/|mailto:|www\.)\S/i.test(text) || /^[^\s@:/]+@[^\s@:/]+\.[^\s@:/]+$/.test(text);
}

/*
 * The Markdown schema in groups; `src/extensions/builtin.ts` registers them
 * as editor features in this order (Tiptap and the Markdown tokenizers
 * depend on it), and `markdownExtensions()` reads them back.
 */

/** Paragraphs, headings, emphasis, code, quotes and rules. */
export function coreTextExtensions(): AnyExtension[] {
  return [
    StarterKit.configure({
      // An empty paragraph at the end would change the saved file.
      trailingNode: false,
      // Replaced by the Markdown rules of Project Write (markdownRules.ts).
      document: false,
      paragraph: false,
      heading: false,
      hardBreak: false,
      code: false,
      codeBlock: false,
      bulletList: false,
      orderedList: false,
      listItem: false,
      link: false,
    }),
    MarkdownDocument,
    MarkdownParagraph,
    MarkdownHeading,
    MarkdownHardBreak,
    MarkdownCode,
    MarkdownCodeBlock,
  ];
}

export function listExtensions(): AnyExtension[] {
  return [MarkdownBulletList, MarkdownOrderedList, MarkdownListItem];
}

/** Web and internal links, and images inside links. */
export function linkExtensions(): AnyExtension[] {
  return [
    MarkdownLink.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      shouldAutoLink: isWebAddress,
      protocols: [...LINK_PROTOCOLS, 'pwrite', 'pwmedia'],
      HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: null },
    }),
    LinkedImages,
  ];
}

/** Character references such as `&amp;`. */
export function entityExtensions(): AnyExtension[] {
  return [Entities];
}

export function taskListExtensions(): AnyExtension[] {
  return [MarkdownTaskList, MarkdownTaskItem.configure({ nested: true })];
}

export function tableExtensions(): AnyExtension[] {
  return [TableKit.configure({ table: false }), MarkdownTable.configure({ resizable: false })];
}

export function imageExtensions(): AnyExtension[] {
  return [Image];
}

/** Raw HTML and footnotes, kept as atoms. */
export function rawExtensions(): AnyExtension[] {
  return [RawBlock, RawInline, Footnotes];
}

/** Parts of the text in another language (the `lang` mark). */
export function languagePartExtensions(): AnyExtension[] {
  return [LanguagePart];
}

/** The Markdown schema (also the schema of `.pwrite` rich text), from the editor features. */
export function markdownExtensions(): AnyExtension[] {
  return schemaExtensions('markdown');
}
