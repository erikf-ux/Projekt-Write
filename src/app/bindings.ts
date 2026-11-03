/** The shortcuts in effect: defaults plus the user's overrides. */
import { useCallback, useMemo } from 'react';
import { useLocale } from '@/i18n';
import { effectiveKeybindings, formatChord } from '@/modules/commands/keymap';
import { allCommands } from '@/modules/commands/registry';
import type { CommandId } from '@/modules/commands/types';
import { useUi } from '@/modules/settings/uiStore';

/** For code outside React (the key listener). */
export function currentBindings(): ReadonlyMap<CommandId, readonly string[]> {
  return effectiveKeybindings(
    allCommands().map((command) => command.id),
    useUi.getState().state?.settings.keybindings ?? {},
  );
}

export function useBindings(): ReadonlyMap<CommandId, readonly string[]> {
  const overrides = useUi((state) => state.state?.settings.keybindings);
  return useMemo(
    () =>
      effectiveKeybindings(
        allCommands().map((command) => command.id),
        overrides ?? {},
      ),
    [overrides],
  );
}

/** The first shortcut of a command as shown to the user, if any. */
export function useShortcutLabel(): (id: CommandId) => string | undefined {
  const bindings = useBindings();
  const locale = useLocale((state) => state.locale);
  const platform = useUi((state) => state.state?.platform ?? 'windows');
  return useCallback(
    (id: CommandId) => {
      const chord = bindings.get(id)?.[0];
      return chord ? formatChord(chord, locale, platform) : undefined;
    },
    [bindings, locale, platform],
  );
}
