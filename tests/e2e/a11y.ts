/**
 * Accessibility rules checked in the running app (section 6, section 11:
 * "Sichtbarer Fokus, ARIA-Beschriftungen"; Phase 14 "Prüfung der
 * Barrierefreiheit").
 *
 * The project adds no rule engine (axe-core would be a new dependency); the
 * rules below are the ones of WCAG 2.2 and WAI-ARIA 1.2 that apply to this
 * interface, checked on what is on screen:
 *
 * * every role is a WAI-ARIA 1.2 role, every `aria-*` attribute exists and
 *   has a valid value, every id it refers to exists, no id is used twice;
 * * every interactive element, form field, dialog and writing surface has
 *   an accessible name (form fields from a label, `aria-label` or
 *   `aria-labelledby`, not only a placeholder);
 * * required states (combobox: `aria-expanded`, checkbox and switch:
 *   `aria-checked`, focusable separator and slider: `aria-valuenow`) and
 *   required context (option in a listbox, tab in a tablist, tree item in a
 *   tree, menu item in a menu) are there; a tablist, tree, menu or listbox
 *   holds no other widgets;
 * * no interactive element inside another, no focusable element inside
 *   `aria-hidden`, no positive `tabindex`, images have a text alternative,
 *   the page has a language.
 *
 * Text contrast (4.5:1) and the contrast of icons and controls (3:1) are
 * measured on the rendered pixels (`renderedContrast`), like the start
 * screen in smoke.e2e.ts.
 */
import { browser } from '@wdio/globals';
import { contrastRatio } from '../../src/modules/themes/contrast';
import { pixelHex, type Image } from './png';

/** Non-abstract roles of WAI-ARIA 1.2 and the Graphics module. */
const ROLES = [
  'alert',
  'alertdialog',
  'application',
  'article',
  'banner',
  'blockquote',
  'button',
  'caption',
  'cell',
  'checkbox',
  'code',
  'columnheader',
  'combobox',
  'complementary',
  'contentinfo',
  'definition',
  'deletion',
  'dialog',
  'document',
  'emphasis',
  'feed',
  'figure',
  'form',
  'generic',
  'graphics-document',
  'graphics-object',
  'graphics-symbol',
  'grid',
  'gridcell',
  'group',
  'heading',
  'img',
  'insertion',
  'link',
  'list',
  'listbox',
  'listitem',
  'log',
  'main',
  'marquee',
  'math',
  'menu',
  'menubar',
  'menuitem',
  'menuitemcheckbox',
  'menuitemradio',
  'meter',
  'navigation',
  'none',
  'note',
  'option',
  'paragraph',
  'presentation',
  'progressbar',
  'radio',
  'radiogroup',
  'region',
  'row',
  'rowgroup',
  'rowheader',
  'scrollbar',
  'search',
  'searchbox',
  'separator',
  'slider',
  'spinbutton',
  'status',
  'strong',
  'subscript',
  'superscript',
  'switch',
  'tab',
  'table',
  'tablist',
  'tabpanel',
  'term',
  'textbox',
  'time',
  'timer',
  'toolbar',
  'tooltip',
  'tree',
  'treegrid',
  'treeitem',
];

/** `aria-*` attributes of WAI-ARIA 1.2 with the values they take ('' = any text, '#' = a number). */
const ATTRIBUTES: Record<string, string[] | ''> = {
  'aria-activedescendant': '',
  'aria-atomic': ['true', 'false'],
  'aria-autocomplete': ['inline', 'list', 'both', 'none'],
  'aria-braillelabel': '',
  'aria-brailleroledescription': '',
  'aria-busy': ['true', 'false'],
  'aria-checked': ['true', 'false', 'mixed', 'undefined'],
  'aria-colcount': ['#'],
  'aria-colindex': ['#'],
  'aria-colindextext': '',
  'aria-colspan': ['#'],
  'aria-controls': '',
  'aria-current': ['page', 'step', 'location', 'date', 'time', 'true', 'false'],
  'aria-describedby': '',
  'aria-description': '',
  'aria-details': '',
  'aria-disabled': ['true', 'false'],
  'aria-errormessage': '',
  'aria-expanded': ['true', 'false', 'undefined'],
  'aria-flowto': '',
  'aria-haspopup': ['false', 'true', 'menu', 'listbox', 'tree', 'grid', 'dialog'],
  'aria-hidden': ['true', 'false', 'undefined'],
  'aria-invalid': ['grammar', 'false', 'spelling', 'true'],
  'aria-keyshortcuts': '',
  'aria-label': '',
  'aria-labelledby': '',
  'aria-level': ['#'],
  'aria-live': ['assertive', 'off', 'polite'],
  'aria-modal': ['true', 'false'],
  'aria-multiline': ['true', 'false'],
  'aria-multiselectable': ['true', 'false'],
  'aria-orientation': ['horizontal', 'vertical', 'undefined'],
  'aria-owns': '',
  'aria-placeholder': '',
  'aria-posinset': ['#'],
  'aria-pressed': ['true', 'false', 'mixed', 'undefined'],
  'aria-readonly': ['true', 'false'],
  'aria-relevant': '',
  'aria-required': ['true', 'false'],
  'aria-roledescription': '',
  'aria-rowcount': ['#'],
  'aria-rowindex': ['#'],
  'aria-rowindextext': '',
  'aria-rowspan': ['#'],
  'aria-selected': ['true', 'false', 'undefined'],
  'aria-setsize': ['#'],
  'aria-sort': ['ascending', 'descending', 'none', 'other'],
  'aria-valuemax': ['#'],
  'aria-valuemin': ['#'],
  'aria-valuenow': ['#'],
  'aria-valuetext': '',
};

/**
 * Deliberate exceptions, each with its reason. Matched against the
 * problem text.
 */
export const ALLOWED_PROBLEMS: { pattern: RegExp; reason: string }[] = [];

/**
 * Checks the rules above on every element of `scope` that is on screen and
 * returns the problems found (empty: none), one line each.
 */
export async function ariaProblems(scope = 'body'): Promise<string[]> {
  const problems = await browser.execute(
    (selector: string, roles: string[], attributes: Record<string, string[] | ''>) => {
      const found: string[] = [];
      const root = document.querySelector(selector);
      if (!root) {
        return [`${selector}: not found`];
      }
      const describe = (element: Element) => {
        const html = element.outerHTML.replace(/\s+/g, ' ');
        const end = html.indexOf('>');
        return (end > 0 && end < 160 ? html.slice(0, end + 1) : html.slice(0, 160)).trim();
      };
      const shown = (element: Element) => {
        if (!(element instanceof HTMLElement || element instanceof SVGElement)) {
          return false;
        }
        if (element.closest('[hidden]') || element.getClientRects().length === 0) {
          return false;
        }
        return getComputedStyle(element).visibility !== 'hidden';
      };
      const role = (element: Element) => element.getAttribute('role')?.trim().split(/\s+/)[0] ?? '';
      const ids = new Map<string, number>();
      for (const element of document.querySelectorAll('[id]')) {
        ids.set(element.id, (ids.get(element.id) ?? 0) + 1);
      }
      for (const [id, count] of ids) {
        if (count > 1 && root.querySelector(`[id="${CSS.escape(id)}"]`)) {
          found.push(`duplicate id "${id}" (${String(count)} times)`);
        }
      }
      if (!document.documentElement.lang) {
        found.push('html has no lang attribute');
      }

      /** Text of an element for its accessible name, without hidden parts. */
      const text = (element: Element): string => {
        let result = '';
        for (const node of element.childNodes) {
          if (node.nodeType === Node.TEXT_NODE) {
            result += node.textContent ?? '';
          } else if (node instanceof Element) {
            if (node.getAttribute('aria-hidden') === 'true') {
              continue;
            }
            if (node instanceof HTMLImageElement) {
              result += node.alt;
            } else if (node.getAttribute('aria-label')) {
              result += node.getAttribute('aria-label') ?? '';
            } else {
              result += text(node);
            }
          }
        }
        return result;
      };
      const referenced = (element: Element, attribute: string) =>
        (element.getAttribute(attribute) ?? '')
          .split(/\s+/)
          .filter(Boolean)
          .map((id) => document.getElementById(id));
      /** The accessible name, simplified from "Accessible Name and Description Computation 1.2". */
      const accessibleName = (element: Element): { name: string; from: string } => {
        const labelledBy = referenced(element, 'aria-labelledby')
          .map((target) => (target ? (target.getAttribute('aria-label') ?? text(target)) : ''))
          .join(' ')
          .trim();
        if (labelledBy) {
          return { name: labelledBy, from: 'aria-labelledby' };
        }
        const label = element.getAttribute('aria-label')?.trim();
        if (label) {
          return { name: label, from: 'aria-label' };
        }
        if (
          element instanceof HTMLInputElement ||
          element instanceof HTMLSelectElement ||
          element instanceof HTMLTextAreaElement
        ) {
          const labels = [...(element.labels ?? [])]
            .map((entry) => text(entry))
            .join(' ')
            .trim();
          if (labels) {
            return { name: labels, from: 'label' };
          }
          if (element instanceof HTMLInputElement && ['button', 'submit', 'reset'].includes(element.type)) {
            return { name: element.value, from: 'value' };
          }
        } else {
          const own = text(element).trim();
          if (own) {
            return { name: own, from: 'content' };
          }
        }
        const title = element.getAttribute('title')?.trim();
        if (title) {
          return { name: title, from: 'title' };
        }
        const placeholder = element.getAttribute('placeholder')?.trim();
        if (placeholder) {
          return { name: placeholder, from: 'placeholder' };
        }
        return { name: '', from: '' };
      };

      const elements = [root, ...root.querySelectorAll('*')].filter(shown);
      /** In the accessibility tree: not inside `aria-hidden`. */
      const exposed = (element: Element) => element.closest('[aria-hidden="true"]') === null;
      const interactiveRoles = new Set([
        'button',
        'link',
        'checkbox',
        'radio',
        'switch',
        'tab',
        'menuitem',
        'menuitemcheckbox',
        'menuitemradio',
        'option',
        'treeitem',
        'combobox',
        'textbox',
        'searchbox',
        'slider',
        'spinbutton',
        'listbox',
        'tree',
        'grid',
        'menu',
        'tablist',
        'dialog',
        'alertdialog',
        'separator',
      ]);
      const isInteractive = (element: Element) => {
        if (element.matches('button, a[href], select, textarea, input:not([type="hidden"])')) {
          return true;
        }
        if (
          element instanceof HTMLElement &&
          element.isContentEditable &&
          element.contentEditable === 'true'
        ) {
          return true;
        }
        const own = role(element);
        if (own === 'separator') {
          return element instanceof HTMLElement && element.tabIndex >= 0;
        }
        return interactiveRoles.has(own);
      };
      const widget = (element: Element) =>
        element.matches('button, a[href], select, textarea, input:not([type="hidden"])') ||
        [
          'button',
          'link',
          'checkbox',
          'radio',
          'switch',
          'textbox',
          'searchbox',
          'combobox',
          'slider',
        ].includes(role(element));
      const context: Record<string, string[]> = {
        option: ['listbox', 'combobox', 'group'],
        tab: ['tablist'],
        treeitem: ['tree', 'group'],
        menuitem: ['menu', 'menubar', 'group'],
        menuitemcheckbox: ['menu', 'menubar', 'group'],
        menuitemradio: ['menu', 'menubar', 'group'],
      };
      const containers = new Set(['tablist', 'tree', 'menu', 'menubar', 'listbox']);

      for (const element of elements) {
        const where = describe(element);
        const own = role(element);
        if (element.hasAttribute('role') && !roles.includes(own)) {
          found.push(`unknown role "${element.getAttribute('role') ?? ''}": ${where}`);
        }
        for (const attribute of element.getAttributeNames()) {
          if (!attribute.startsWith('aria-')) {
            continue;
          }
          const allowed = attributes[attribute];
          const value = element.getAttribute(attribute)?.trim() ?? '';
          if (allowed === undefined) {
            found.push(`unknown attribute ${attribute}: ${where}`);
          } else if (allowed !== '' && allowed[0] === '#') {
            if (value === '' || !Number.isFinite(Number(value))) {
              found.push(`${attribute}="${value}" is no number: ${where}`);
            }
          } else if (allowed !== '' && !allowed.includes(value)) {
            found.push(`${attribute}="${value}" is not one of ${allowed.join(', ')}: ${where}`);
          }
          if (
            [
              'aria-labelledby',
              'aria-describedby',
              'aria-controls',
              'aria-owns',
              'aria-activedescendant',
              'aria-errormessage',
              'aria-details',
              'aria-flowto',
            ].includes(attribute)
          ) {
            const missing = value.split(/\s+/).filter((id) => id !== '' && !document.getElementById(id));
            if (missing.length > 0) {
              found.push(`${attribute} refers to missing id ${missing.join(', ')}: ${where}`);
            }
          }
        }
        // An image with alt="" is decorative (its text stands next to it).
        if (
          exposed(element) &&
          (isInteractive(element) || element.matches('img:not([alt=""]), [role="img"]'))
        ) {
          const { name, from } = accessibleName(element);
          if (!name) {
            found.push(`no accessible name: ${where}`);
          } else if (from === 'placeholder') {
            found.push(`named by its placeholder only ("${name}"): ${where}`);
          }
        }
        if (element instanceof HTMLImageElement && !element.hasAttribute('alt')) {
          found.push(`image without alt: ${where}`);
        }
        if (own === 'combobox' && !element.hasAttribute('aria-expanded')) {
          found.push(`combobox without aria-expanded: ${where}`);
        }
        if (['checkbox', 'switch', 'menuitemcheckbox', 'menuitemradio', 'radio'].includes(own)) {
          if (!(element instanceof HTMLInputElement) && !element.hasAttribute('aria-checked')) {
            found.push(`${own} without aria-checked: ${where}`);
          }
        }
        if (
          (own === 'slider' ||
            (own === 'separator' && element instanceof HTMLElement && element.tabIndex >= 0)) &&
          !element.hasAttribute('aria-valuenow')
        ) {
          found.push(`${own} without aria-valuenow: ${where}`);
        }
        if (own === 'heading' && !/^H[1-6]$/.test(element.tagName) && !element.hasAttribute('aria-level')) {
          found.push(`heading without aria-level: ${where}`);
        }
        const needed = context[own];
        if (needed) {
          let parent = element.parentElement;
          while (
            parent &&
            (!parent.hasAttribute('role') || ['none', 'presentation', 'generic'].includes(role(parent)))
          ) {
            parent = parent.parentElement;
          }
          if (!parent || !needed.includes(role(parent))) {
            found.push(
              `${own} outside ${needed.join(' or ')} (in ${parent ? role(parent) : 'nothing'}): ${where}`,
            );
          }
        }
        if (containers.has(own)) {
          for (const inner of element.querySelectorAll('*')) {
            if (
              shown(inner) &&
              exposed(inner) &&
              widget(inner) &&
              !(
                inner instanceof HTMLElement &&
                inner.closest('[role="option"], [role="tab"], [role="treeitem"], [role^="menuitem"]')
              )
            ) {
              found.push(`${own} holds a widget that is none of its items: ${describe(inner)}`);
            }
          }
        }
        if (element instanceof HTMLElement && element.tabIndex > 0) {
          found.push(`positive tabindex ${String(element.tabIndex)}: ${where}`);
        }
        if (element.getAttribute('aria-hidden') === 'true') {
          for (const inner of [element, ...element.querySelectorAll('*')]) {
            if (
              inner instanceof HTMLElement &&
              inner.tabIndex >= 0 &&
              !(inner as HTMLButtonElement).disabled &&
              (inner.matches('button, a[href], input, select, textarea') || inner.hasAttribute('tabindex'))
            ) {
              found.push(`focusable inside aria-hidden: ${describe(inner)}`);
            }
          }
        }
        if (widget(element) && exposed(element)) {
          const outer = element.parentElement?.closest(
            'button, a[href], [role="button"], [role="link"], [role="tab"], [role="option"], [role="menuitem"], [role="treeitem"]',
          );
          if (outer && root.contains(outer)) {
            found.push(`interactive inside interactive (${describe(outer)}): ${where}`);
          }
        }
      }
      return [...new Set(found)];
    },
    scope,
    ROLES,
    ATTRIBUTES,
  );
  return problems.filter((problem) => !ALLOWED_PROBLEMS.some((entry) => entry.pattern.test(problem)));
}

/** A box on screen (CSS pixels). */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Rendered contrast of one box: background is the most frequent pixel in
 * the box, the foreground the pixel with the highest contrast against it
 * (glyph cores and icon strokes reach their full colour).
 */
export function renderedContrast(image: Image, scale: number, box: Box) {
  const counts = new Map<string, number>();
  const pixels: string[] = [];
  for (let y = Math.ceil(box.y * scale); y < Math.floor((box.y + box.height) * scale); y++) {
    for (let x = Math.ceil(box.x * scale); x < Math.floor((box.x + box.width) * scale); x++) {
      if (x < 0 || y < 0 || x >= image.width || y >= image.height) {
        continue;
      }
      const pixel = pixelHex(image, x, y);
      pixels.push(pixel);
      counts.set(pixel, (counts.get(pixel) ?? 0) + 1);
    }
  }
  const background = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? '#000000';
  let foreground = background;
  let ratio = 1;
  for (const pixel of pixels) {
    const candidate = contrastRatio(pixel, background);
    if (candidate > ratio) {
      ratio = candidate;
      foreground = pixel;
    }
  }
  return { foreground, background, ratio };
}

export interface ScreenItem extends Box {
  kind: 'text' | 'graphic';
  label: string;
}

/**
 * Every text on screen inside `scope` (one box per line of a text node)
 * and every icon, checkbox and radio button, where it is the topmost
 * element. Disabled controls are left out (WCAG 1.4.3 and 1.4.11 exempt
 * them).
 */
export async function screenItems(scope = 'body'): Promise<ScreenItem[]> {
  return browser.execute((selector: string) => {
    const root = document.querySelector(selector);
    if (!root) {
      return [];
    }
    const items: {
      kind: 'text' | 'graphic';
      label: string;
      x: number;
      y: number;
      width: number;
      height: number;
    }[] = [];
    const visible = (element: Element) =>
      !element.closest('[hidden], [aria-disabled="true"], :disabled, .sr-only') &&
      getComputedStyle(element).visibility !== 'hidden' &&
      Number(getComputedStyle(element).opacity) > 0;
    /** Whether the element is what the user sees at the middle of the box. */
    const onTop = (element: Element, box: DOMRect) => {
      const x = box.left + box.width / 2;
      const y = box.top + box.height / 2;
      if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) {
        return false;
      }
      const hit = document.elementFromPoint(x, y);
      return hit !== null && element.contains(hit);
    };
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const parent = node.parentElement;
      const content = node.textContent?.trim() ?? '';
      if (!parent || content === '' || /^[\p{P}\s]+$/u.test(content) || !visible(parent)) {
        continue;
      }
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const box of range.getClientRects()) {
        if (box.width >= 2 && box.height >= 6 && onTop(parent, box)) {
          items.push({
            kind: 'text',
            label: content.slice(0, 40),
            x: box.left,
            y: box.top,
            width: box.width,
            height: box.height,
          });
        }
      }
    }
    for (const element of root.querySelectorAll('svg, input[type="checkbox"], input[type="radio"]')) {
      if (element.closest('svg') !== element && element.tagName.toLowerCase() === 'svg') {
        continue;
      }
      const box = element.getBoundingClientRect();
      if (box.width < 6 || box.height < 6 || !visible(element) || !onTop(element, box)) {
        continue;
      }
      const owner = element.closest('[aria-label], [title], label, button');
      items.push({
        kind: 'graphic',
        label: `${element.tagName.toLowerCase()} ${owner?.getAttribute('aria-label') ?? owner?.getAttribute('title') ?? owner?.textContent.trim().slice(0, 30) ?? ''}`,
        x: box.left,
        y: box.top,
        width: box.width,
        height: box.height,
      });
    }
    return items;
  }, scope);
}
