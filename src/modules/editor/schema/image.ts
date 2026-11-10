/**
 * Images in documents (section 10/12). An image of the library
 * (`pwmedia://<id>`, Phase 8, ADR 0019 decision 10) is shown through the
 * media handler, with its caption (the title) below and its display size;
 * if the medium is gone, "Medium fehlt" takes its place. Every other image
 * stays a note with its address and is never loaded (section 12: "Bilder
 * mit Webadresse werden nie automatisch geladen. Sie erscheinen als Hinweis
 * mit Adresse.").
 *
 * Size is part of the allowed HTML subset: an image with `width` or
 * `height` is written as `<img>`, otherwise as `![alt](src "title")`.
 */
import { Node, type JSONContent } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { NodeSelection } from '@tiptap/pm/state';
import { translate, useLocale } from '@/i18n';
import { mediaUrl, parseMediaReference } from '@/modules/media/mediaUrl';
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

/**
 * A width or height of the allowed subset as CSS: a percentage of the line
 * (`50%`) or pixels (`320`, `320px`); anything else is not applied.
 */
export function cssSize(value: string | null): string | null {
  const text = value?.trim() ?? '';
  if (
    /^\d{1,3}(?:\.\d{1,2})?%$/.test(text) &&
    Number.parseFloat(text) > 0 &&
    Number.parseFloat(text) <= 100
  ) {
    return text;
  }
  const pixels = /^(\d{1,5})(?:px)?$/.exec(text)?.[1];
  return pixels && Number(pixels) > 0 ? `${pixels}px` : null;
}

/** Opens the dialog of an image (double click); set by the editor module. */
let editImage: ((position: number) => void) | null = null;

export function setImageEditor(handler: ((position: number) => void) | null): void {
  editImage = handler;
}

function noteContent(dom: HTMLElement, image: ImageAttributes) {
  dom.className = 'pw-image';
  dom.setAttribute('data-pw-image', '');
  dom.title = image.title ?? image.src;
  dom.textContent = image.alt === '' ? image.src : image.alt;
}

/** An image of the library: the picture through the handler, the caption below. */
function mediaContent(dom: HTMLElement, image: ImageAttributes, id: string) {
  dom.className = 'pw-media-image';
  dom.setAttribute('data-pw-media-image', id);
  dom.removeAttribute('title');
  dom.style.width = cssSize(image.width) ?? '';
  const picture = document.createElement('img');
  picture.src = mediaUrl(id);
  picture.alt = image.alt;
  picture.draggable = false;
  const height = image.width === null ? cssSize(image.height) : null;
  if (height) {
    picture.style.height = height;
  }
  picture.addEventListener('error', () => {
    const missing = document.createElement('span');
    missing.className = 'pw-media-missing';
    missing.setAttribute('data-pw-media-missing', '');
    missing.textContent = translate(useLocale.getState().locale, 'editor.mediaMissing');
    picture.replaceWith(missing);
  });
  const parts: HTMLElement[] = [picture];
  if (image.title !== null && image.title !== '') {
    const caption = document.createElement('span');
    caption.className = 'pw-media-caption';
    caption.textContent = image.title;
    parts.push(caption);
  }
  dom.replaceChildren(...parts);
}

function renderInto(dom: HTMLElement, node: ProseMirrorNode) {
  const image = attributes({ type: node.type.name, attrs: node.attrs });
  for (const name of ['class', 'style', 'data-pw-image', 'data-pw-media-image']) {
    dom.removeAttribute(name);
  }
  dom.replaceChildren();
  const medium = parseMediaReference(image.src);
  if (medium && medium.page === null) {
    mediaContent(dom, image, medium.id);
  } else {
    noteContent(dom, image);
  }
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

  // In the editor: library images as pictures, everything else as a note.
  // `renderHTML` above stays for the clipboard.
  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement('span');
      dom.contentEditable = 'false';
      let current = node;
      renderInto(dom, current);
      // A click selects the image (outline; "Bild bearbeiten" then works
      // on it), whatever the webview makes of a click on a picture.
      dom.addEventListener('mousedown', (event) => {
        const position = typeof getPos === 'function' ? getPos() : undefined;
        if (event.button !== 0 || position === undefined || !editor.isEditable) {
          return;
        }
        event.preventDefault();
        const { view } = editor;
        view.focus();
        view.dispatch(view.state.tr.setSelection(NodeSelection.create(view.state.doc, position)));
      });
      dom.addEventListener('dblclick', (event) => {
        const position = typeof getPos === 'function' ? getPos() : undefined;
        if (position !== undefined && dom.hasAttribute('data-pw-media-image') && editImage) {
          event.preventDefault();
          editImage(position);
        }
      });
      return {
        dom,
        update: (next) => {
          if (next.type !== current.type) {
            return false;
          }
          if (next.attrs !== current.attrs) {
            current = next;
            renderInto(dom, current);
          }
          return true;
        },
        // The picture loading or failing is no change of the document.
        ignoreMutation: () => true,
        // The click above selects the image; ProseMirror would put a text
        // selection next to it on mouse up instead.
        stopEvent: (event) => event.type === 'mousedown',
      };
    };
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
