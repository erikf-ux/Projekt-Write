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
  MarkdownTaskItem,
  MarkdownTaskList,
} from './markdownRules';
import { Footnotes, RawBlock, RawInline } from './raw';

/** Web links the app may open (section 12). */
export const LINK_PROTOCOLS = ['http', 'https', 'mailto'] as const;

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
      protocols: [...LINK_PROTOCOLS],
      HTMLAttributes: { rel: 'noopener noreferrer nofollow', target: null },
    }),
    LinkedImages,
    Entities,
    MarkdownTaskList,
    MarkdownTaskItem.configure({ nested: true }),
    TableKit.configure({ table: { resizable: false } }),
    Image,
    RawBlock,
    RawInline,
    Footnotes,
  ];
}
