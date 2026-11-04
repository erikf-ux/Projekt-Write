/**
 * Commands of Phases 3 and 4. Menu, toolbar, command palette, shortcuts and context
 * menus all run commands from one registry (ADR 0012), so every way to
 * reach a function behaves the same.
 */
import type { MessageKey } from '@/i18n';
import type { MenuCommand } from '@/types/generated/MenuCommand';

export type CommandId =
  | 'document.new'
  | 'document.save'
  | 'document.rename'
  | 'document.copy'
  | 'document.toggleFavorite'
  | 'tab.close'
  | 'tab.closeOthers'
  | 'tab.closeAll'
  | 'tab.next'
  | 'tab.previous'
  | 'tab.moveLeft'
  | 'tab.moveRight'
  | 'palette.show'
  | 'settings.open'
  | 'settings.shortcuts'
  | 'app.quit'
  | 'sidebar.toggle'
  | 'explorer.focus'
  | 'editor.focus'
  | 'editor.toggleSource'
  | 'statistics.show'
  | 'find.show'
  | 'replace.show'
  | 'find.next'
  | 'find.previous'
  | 'edit.undo'
  | 'edit.redo'
  | 'format.bold'
  | 'format.italic'
  | 'format.underline'
  | 'format.strike'
  | 'format.code'
  | 'format.paragraph'
  | 'format.heading1'
  | 'format.heading2'
  | 'format.heading3'
  | 'format.bulletList'
  | 'format.orderedList'
  | 'format.taskList'
  | 'format.blockquote'
  | 'format.codeBlock'
  | 'format.link'
  | 'format.horizontalRule'
  | 'format.table'
  | 'focus.toggle'
  | 'zoom.in'
  | 'zoom.out'
  | 'zoom.reset'
  | 'theme.system'
  | 'theme.light'
  | 'theme.dark'
  | 'lineWidth.toggle';

/** Every menu item of the native menu is a command (checked by the compiler). */
export type MenuCommandIsCommand = MenuCommand extends CommandId ? true : never;
export const MENU_COMMANDS_ARE_COMMANDS: MenuCommandIsCommand = true;

export type CommandCategory = 'document' | 'edit' | 'tab' | 'view' | 'app';

export interface CommandDefinition {
  id: CommandId;
  category: CommandCategory;
  /** Title in the command palette, the menu and the shortcut settings. */
  titleKey: MessageKey;
  /** Whether the command works right now; unavailable commands stay hidden. */
  isAvailable: () => boolean;
  run: () => void | Promise<void>;
}
