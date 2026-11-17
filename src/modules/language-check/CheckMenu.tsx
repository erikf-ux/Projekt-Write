/**
 * The context menu of a finding (section 14): suggestions (at most five,
 * in the order of section 14), "add to dictionary" with one click and
 * "ignore"; for the rules their replacement. Opened with the right mouse
 * button, the context menu key or `Shift + F10`.
 */
import type { EditorView } from '@tiptap/pm/view';
import { create } from 'zustand';
import { ContextMenu, type ContextMenuItem } from '@/components/ContextMenu';
import { translate, useLocale, type MessageKey } from '@/i18n';
import { call } from '@/services/ipc';
import { useLanguage } from './languageStore';
import { findingAt, replaceFinding, type FindingSpec } from './plugin';

interface MenuState {
  x: number;
  y: number;
  items: ContextMenuItem[];
}

const useCheckMenu = create<{ menu: MenuState | null }>(() => ({ menu: null }));

function t(key: MessageKey, params?: Record<string, string | number>): string {
  return translate(useLocale.getState().locale, key, params);
}

function replacementLabel(finding: FindingSpec, replacement: string): string {
  if (finding.finding.rule === 'doubleWord') {
    return t('language.menu.removeRepeat');
  }
  if (finding.finding.rule === 'doubleSpace') {
    return t('language.menu.oneSpace');
  }
  return replacement;
}

function apply(view: EditorView, finding: FindingSpec, replacement: string): void {
  replaceFinding(view, finding, replacement);
  view.focus();
}

function spellingItems(view: EditorView, finding: FindingSpec, word: string, suggestions: string[] | null) {
  const items: ContextMenuItem[] = [];
  if (suggestions === null) {
    items.push({ id: 'searching', label: t('language.menu.searching'), onSelect: () => undefined });
  } else if (suggestions.length === 0) {
    items.push({ id: 'none', label: t('language.menu.none'), onSelect: () => undefined });
  } else {
    suggestions.forEach((suggestion, index) => {
      items.push({
        id: `suggestion-${String(index)}`,
        label: suggestion,
        onSelect: () => {
          apply(view, finding, suggestion);
        },
      });
    });
  }
  items.push({
    id: 'add',
    label: t('language.menu.add', { word }),
    onSelect: () => {
      void call('dictionary_add_word', { word, defaultName: t('language.ownWordsName') }).then((result) => {
        if (result.ok) {
          useLanguage.getState().bump();
        }
      });
    },
  });
  items.push({
    id: 'ignore',
    label: t('language.menu.ignore'),
    onSelect: () => {
      useLanguage.getState().ignore(word);
    },
  });
  return items;
}

/**
 * Opens the menu for the finding at `position`; false when there is none
 * (the caller then shows nothing of its own).
 */
export function openCheckMenu(
  view: EditorView,
  position: number,
  x: number,
  y: number,
  documentId: string | null,
): boolean {
  const finding = findingAt(view.state, position);
  if (!finding) {
    return false;
  }
  const word = view.state.doc.textBetween(finding.from, finding.to);
  if (finding.finding.kind !== 'spelling') {
    const items: ContextMenuItem[] = finding.finding.replacements.map((replacement, index) => ({
      id: `replacement-${String(index)}`,
      label: replacementLabel(finding, replacement),
      onSelect: () => {
        apply(view, finding, replacement);
      },
    }));
    if (items.length === 0) {
      items.push({
        id: 'message',
        label: finding.finding.message ?? t(`language.rule.${finding.finding.rule}` as MessageKey),
        onSelect: () => undefined,
      });
    }
    useCheckMenu.setState({ menu: { x, y, items } });
    return true;
  }
  useCheckMenu.setState({ menu: { x, y, items: spellingItems(view, finding, word, null) } });
  void call('language_suggest', {
    ...(documentId ? { documentId } : {}),
    word,
    language: finding.language,
  }).then((result) => {
    const open = useCheckMenu.getState().menu;
    if (!open || open.x !== x || open.y !== y) {
      return;
    }
    const suggestions = result.ok && Array.isArray(result.value) ? result.value : [];
    useCheckMenu.setState({ menu: { x, y, items: spellingItems(view, finding, word, suggestions) } });
  });
  return true;
}

/** Opens the menu from the keyboard, at the cursor. */
export function openCheckMenuAtCursor(view: EditorView, documentId: string | null): boolean {
  const position = view.state.selection.head;
  const coords = view.coordsAtPos(position);
  return openCheckMenu(view, position, coords.left, coords.bottom, documentId);
}

export function closeCheckMenu(): void {
  useCheckMenu.setState({ menu: null });
}

export function CheckMenu() {
  const menu = useCheckMenu((state) => state.menu);
  if (!menu) {
    return null;
  }
  return (
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={menu.items}
      label={t('language.menu.label')}
      onClose={closeCheckMenu}
    />
  );
}
