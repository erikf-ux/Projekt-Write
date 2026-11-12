/**
 * Paths of the end-to-end tests. Everything the tests and the app write lies
 * in `tests/e2e/.tmp` inside the project folder (section 2).
 */
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const binary = process.platform === 'win32' ? 'project-write.exe' : 'project-write';
/** The app under test; PW_E2E_APP lets experiments (ADR 0005) use another binary. */
export const application =
  process.env['PW_E2E_APP'] ?? path.join(root, 'src-tauri', 'target', 'debug', binary);

export const sandbox = path.join(root, 'tests', 'e2e', '.tmp');

/**
 * App folders of the e2e build (config, logs, documents; `PW_E2E_APP_DIRS`).
 * Named like a sync client's folder, so the default library location triggers
 * the sync folder warning (section 9).
 */
export const appDirs = path.join(sandbox, 'OneDrive');

/** Links the e2e build "opens" (`PW_E2E_OPENED_LINKS`), one per line. */
export const openedLinks = path.join(sandbox, 'opened-links.txt');

/** Answers of the folder dialog, in this order, at every app start. */
export const pickedFolders = {
  local: path.join(sandbox, 'Lokal', 'Bibliothek'),
  moveTarget: path.join(appDirs, 'Umzug'),
};

/** Files outside the library and the answers of the file dialog, in this order. */
const ownFiles = path.join(sandbox, 'Eigene Dateien');
export const pickedFiles = {
  utf8: path.join(ownFiles, 'utf8.txt'),
  utf8Bom: path.join(ownFiles, 'utf8-bom.txt'),
  utf16le: path.join(ownFiles, 'utf16le.txt'),
  utf16be: path.join(ownFiles, 'utf16be.txt'),
  windows1252: path.join(ownFiles, 'windows-1252.txt'),
  unchanged: path.join(ownFiles, 'unveraendert.txt'),
  outside: path.join(ownFiles, 'aussen.txt'),
  draft: path.join(ownFiles, 'entwurf.txt'),
};

/** Originals of the media import (Phase 8), outside the library. */
export const mediaOriginals = path.join(sandbox, 'Medien');

/** Answers of the media import dialogs (`PW_E2E_PICK_MEDIA`), in this order. */
export const pickedMedia = {
  /** A folder with subfolders, refused files, an SVG with script and the CVE test PDF. */
  folder: path.join(mediaOriginals, 'Reise'),
  /** One file with the content of a medium already imported. */
  duplicate: path.join(mediaOriginals, 'Kopie von Ankunft.png'),
  /** A folder whose import is cancelled. */
  cancelled: path.join(mediaOriginals, 'Abbruch'),
};

/** Exports and backups (Phase 10), outside the library. */
export const exportsDir = path.join(sandbox, 'Export');
export const backupsDir = path.join(sandbox, 'Sicherungen');
export const restoreParent = path.join(sandbox, 'Wiederherstellung');

/** Answers of the export and backup dialogs (`PW_E2E_PICK_EXPORT`), in this order. */
export const pickedExport = {
  markdown: path.join(exportsDir, 'Bericht.md'),
  text: path.join(exportsDir, 'Bericht.txt'),
  pack: path.join(exportsDir, 'Bericht.pwritepack'),
  packImport: path.join(exportsDir, 'Bericht.pwritepack'),
  library: exportsDir,
  backupTarget: backupsDir,
  check: path.join(backupsDir, 'pruefen.pwbackup'),
  damaged: path.join(backupsDir, 'kaputt.pwbackup'),
  restoreFile: path.join(backupsDir, 'pruefen.pwbackup'),
  restoreFolder: restoreParent,
};

/** A document printed to PDF by the engine of the webview (export-backup.e2e.ts). */
export const printedPdf = path.join(sandbox, 'Drucken', 'Druckprobe.pdf');

/**
 * The temporary folder of the app and the WebDriver (the WebDriver prints to
 * a file there before it hands the PDF over).
 */
export const tempDir = path.join(sandbox, 'tmp');

/** Passed to a second start of the app (not a dialog answer). */
export const secondStartFile = path.join(ownFiles, 'zweiter-start.md');

/** What a spec file that ends the app leaves for the next one to check. */
export const handover = path.join(sandbox, 'handover.json');

/**
 * The reference library of the Explorer measurement (Phase 7). Building it
 * takes minutes, so it is kept between runs, inside the project folder
 * (section 2) but outside the sandbox the launcher clears.
 */
export const referenceCache = path.join(root, 'tests', 'e2e', '.cache');

/** The generator of the reference library (built by `scripts/reference-library.mjs`). */
export const referenceGenerator = path.join(
  root,
  'src-tauri',
  'target',
  'debug',
  'examples',
  process.platform === 'win32' ? 'reference_library.exe' : 'reference_library',
);
