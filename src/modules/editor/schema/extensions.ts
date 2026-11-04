/**
 * The schema of Markdown documents (section 12): everything Markdown can
 * hold, nothing more, so saving never has to drop formatting. Raw HTML and
 * footnotes are kept as atoms (`raw.ts`), images as notes (`image.ts`).
 */
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

/** Web links the app may open (section 12). */
export const LINK_PROTOCOLS = ['http', 'https', 'mailto'] as const;

/**
 * Text that becomes a link by itself, while typing or pasting: addresses
 * with `http://`, `https://`, `mailto:` or `www.`, and e-mail addresses.
 * Not bare domains: in prose a missing space after a full stop ("im.Am",
 * "geht.so") would become a link to a country domain.
 */
export function isWebAddress(text: string): boolean {
  return /^(?:https?:\/\/|mailto:|www\.)\S/i.test(text) || /^[^\s@:/]+@[^\s@:/]+\.[^\s@:/]+$/.test(text);
}

export function markdownExtensions(): AnyExtension[] {
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
    MarkdownBulletList,
    MarkdownOrderedList,
    MarkdownListItem,
    MarkdownLink.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      shouldAutoLink: isWebAddress,
      protocols: [...LINK_PROTOCOLS],
      HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: null },
    }),
    LinkedImages,
    Entities,
    MarkdownTaskList,
    MarkdownTaskItem.configure({ nested: true }),
    TableKit.configure({ table: false }),
    MarkdownTable.configure({ resizable: false }),
    Image,
    RawBlock,
    RawInline,
    Footnotes,
  ];
}
