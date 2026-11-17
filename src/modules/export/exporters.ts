/**
 * Exporters (ADR 0027, decision 2): the export and print actions of the
 * interface. The commands (palette, native menu, shortcuts) are made from
 * the registry (`src/app/commands.ts`); the command ids stay as they were,
 * because the shortcut settings and the native menu name them.
 */
import type { RegistryEntry } from '@/core/registry';
import type { MessageKey } from '@/i18n';
import type { CommandCategory, CommandId } from '@/modules/commands/types';
import { activeDocument, useWorkspace } from '@/modules/documents/workspaceStore';
import { useLibrary } from '@/modules/library/libraryStore';
import { exportActive, exportLibrary, exportPackActive, printActive } from './exportActions';

export interface Exporter extends RegistryEntry {
  /** The command that runs the exporter. */
  readonly command: CommandId;
  readonly category: CommandCategory;
  readonly titleKey: MessageKey;
  /** Whether the exporter works right now (the palette shows only these). */
  readonly isAvailable: () => boolean;
  /** Runs the export; false if it did not happen (cancelled, refused). */
  readonly run: () => Promise<boolean>;
}

/** The active tab shows a loaded document (library or external file). */
const anyDocumentLoaded = () => activeDocument(useWorkspace.getState())?.loaded === true;

/** The active tab shows a loaded library document. */
const libraryDocumentLoaded = () => {
  const document = activeDocument(useWorkspace.getState());
  return document !== null && !document.external && document.loaded;
};

const libraryOpen = () => useLibrary.getState().status?.state === 'open';

export const printExporter: Exporter = {
  id: 'print',
  command: 'document.print',
  category: 'document',
  titleKey: 'command.document.print',
  isAvailable: anyDocumentLoaded,
  run: () => printActive(false),
};

export const pdfExporter: Exporter = {
  id: 'pdf',
  command: 'export.pdf',
  category: 'document',
  titleKey: 'command.export.pdf',
  isAvailable: anyDocumentLoaded,
  run: () => printActive(true),
};

export const txtExporter: Exporter = {
  id: 'txt',
  command: 'export.txt',
  category: 'document',
  titleKey: 'command.export.txt',
  isAvailable: libraryDocumentLoaded,
  run: () => exportActive('txt'),
};

export const markdownExporter: Exporter = {
  id: 'markdown',
  command: 'export.markdown',
  category: 'document',
  titleKey: 'command.export.markdown',
  isAvailable: libraryDocumentLoaded,
  run: () => exportActive('markdown'),
};

export const packExporter: Exporter = {
  id: 'pack',
  command: 'export.pack',
  category: 'document',
  titleKey: 'command.export.pack',
  isAvailable: libraryDocumentLoaded,
  run: () => exportPackActive(),
};

export const libraryExporter: Exporter = {
  id: 'library',
  command: 'export.library',
  category: 'app',
  titleKey: 'command.export.library',
  isAvailable: libraryOpen,
  run: () => exportLibrary(),
};
