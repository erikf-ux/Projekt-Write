/**
 * The command registry: one list of command definitions for the whole app.
 * The palette shows the available ones, shortcuts and the native menu run
 * them by id.
 */
import type { CommandDefinition, CommandId } from './types';

let definitions: readonly CommandDefinition[] = [];

/** Replaces the registered commands (the app registers them once at start). */
export function registerCommands(list: readonly CommandDefinition[]): void {
  const ids = new Set<CommandId>();
  for (const command of list) {
    if (ids.has(command.id)) {
      throw new Error(`Command registered twice: ${command.id}`);
    }
    ids.add(command.id);
  }
  definitions = list;
}

export function allCommands(): readonly CommandDefinition[] {
  return definitions;
}

export function findCommand(id: string): CommandDefinition | undefined {
  return definitions.find((command) => command.id === id);
}

/** Commands that work right now (the palette shows only these). */
export function availableCommands(): CommandDefinition[] {
  return definitions.filter((command) => command.isAvailable());
}

/** Runs a command if it is available; returns whether it ran. */
export async function runCommand(id: string): Promise<boolean> {
  const command = findCommand(id);
  if (!command?.isAvailable()) {
    return false;
  }
  await command.run();
  return true;
}
