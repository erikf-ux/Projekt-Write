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
