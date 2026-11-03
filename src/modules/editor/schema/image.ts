/**
 * Images in Markdown documents (section 10/12). Phase 4 keeps them exactly
 * and shows them as a note with their address; nothing is ever loaded
 * (section 12: "Bilder mit Webadresse werden nie automatisch geladen. Sie
 * erscheinen als Hinweis mit Adresse."). Inserting and importing images
 * comes with the media library (Phase 8).
 *
 * Size is part of the allowed HTML subset: an image with `width` or
 * `height` is written as `<img>`, otherwise as `![alt](src "title")`.
 */
import { Node, type JSONContent } from '@tiptap/core';
import { stringAttribute, tokenString } from './attributes';
import { linkDestination, linkTitle } from '../markdown/escape';

interface ImageAttributes {
  src: string;
  alt: string;
  title: string | null;
  width: string | null;
  height: string | null;
  /** Target of a linked image `[![alt](src)](href)` (section 10). */
  href: string | null;
  linkTitle: string | null;
}

function attributes(node: JSONContent): ImageAttributes {
  const value = (name: string): string | null => stringAttribute(node, name);
  return {
    src: value('src') ?? '',
    alt: value('alt') ?? '',
    title: value('title'),
    width: value('width'),
    height: value('height'),
    href: value('href'),
    linkTitle: value('linkTitle'),
  };
}

function escapeHtmlAttribute(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

/** `![alt](src "title")` with the escapes CommonMark needs. */
export function imageMarkdown({ src, alt, title }: ImageAttributes): string {
  const text = alt.replace(/[\\[\]]/g, '\\$&');
  return `![${text}](${linkDestination(src)}${linkTitle(title)})`;
}

/** `<img …>` of the allowed subset, attributes in a fixed order. */
export function imageHtml(image: ImageAttributes): string {
  const parts = [`src="${escapeHtmlAttribute(image.src)}"`];
  if (image.alt !== '') parts.push(`alt="${escapeHtmlAttribute(image.alt)}"`);
  if (image.title !== null) parts.push(`title="${escapeHtmlAttribute(image.title)}"`);
  if (image.width !== null) parts.push(`width="${escapeHtmlAttribute(image.width)}"`);
  if (image.height !== null) parts.push(`height="${escapeHtmlAttribute(image.height)}"`);
  return `<img ${parts.join(' ')}>`;
}

export const Image = Node.create({
  name: 'image',
  group: 'inline',
  inline: true,
  atom: true,
  draggable: false,
  selectable: true,

  addAttributes() {
    return {
      src: { default: '' },
      alt: { default: '' },
      title: { default: null },
      // Kept as written: Tiptap would turn "100" into a number.
      width: {
        default: null,
        parseHTML: (element: HTMLElement) =>
          element.getAttribute('width') ?? element.getAttribute('data-width'),
      },
      height: {
        default: null,
        parseHTML: (element: HTMLElement) =>
          element.getAttribute('height') ?? element.getAttribute('data-height'),
      },
      href: { default: null },
      linkTitle: { default: null },
    };
  },

  parseHTML() {
    const read = (element: HTMLElement) => ({
      src: element.getAttribute('src') ?? element.getAttribute('data-src') ?? '',
      alt: element.getAttribute('alt') ?? element.getAttribute('data-alt') ?? '',
      title: element.getAttribute('title'),
      width: element.getAttribute('width') ?? element.getAttribute('data-width'),
      height: element.getAttribute('height') ?? element.getAttribute('data-height'),
    });
    return [
      { tag: 'img[src]', getAttrs: read },
      { tag: 'span[data-pw-image]', getAttrs: read },
    ];
  },

  // Rendered as text: the address is shown, never requested.
  renderHTML({ node }) {
    const image = attributes({ type: node.type.name, attrs: node.attrs });
    const label = image.alt === '' ? image.src : image.alt;
    return [
      'span',
      {
        'data-pw-image': '',
        'data-src': image.src,
        'data-alt': image.alt,
        'data-width': image.width,
        'data-height': image.height,
        title: image.title ?? image.src,
        class: 'pw-image',
      },
      label,
    ];
  },

  markdownTokenName: 'image',

  parseMarkdown: (token) => ({
    type: 'image',
    attrs: {
      src: tokenString(token['href']) ?? '',
      alt: token.text ?? '',
      title: tokenString(token['title']),
    },
  }),

  renderMarkdown: (node: JSONContent) => {
    const image = attributes(node);
    const rendered = image.width !== null || image.height !== null ? imageHtml(image) : imageMarkdown(image);
    if (image.href === null) {
      return rendered;
    }
    return `[${rendered}](${linkDestination(image.href)}${linkTitle(image.linkTitle)})`;
  },
});
