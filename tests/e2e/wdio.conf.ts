/**
 * End-to-end tests against the real app binary (debug build with the `e2e`
 * feature, see `npm run build:e2e`). The embedded WebDriver server of
 * `tauri-plugin-wdio-webdriver` works on Windows, macOS and Linux and needs no
 * `withGlobalTauri` (see ADR 0003).
 */
import fs from 'node:fs';
import path from 'node:path';
import {
  appDirs,
  application,
  openedLinks,
  pickedFiles,
  pickedFolders,
  pickedMedia,
  pickedExport,
  root,
  sandbox,
} from './sandbox';

if (!fs.existsSync(application)) {
  throw new Error(`App binary not found at ${application}. Run "npm run build:e2e" first.`);
}

// Keep everything the webview writes (caches, local storage) inside the
// project folder (section 2: tests never write outside it).
// WebdriverIO evaluates this file again in every worker. Only the launcher may
// clear the folder: by then the app is running and, on Windows, holds locks on
// its WebView2 data inside it.
if (!process.env['WDIO_WORKER_ID']) {
  fs.rmSync(sandbox, { recursive: true, force: true });
}
for (const dir of ['data', 'config', 'cache', 'webview2']) {
  fs.mkdirSync(path.join(sandbox, dir), { recursive: true });
}
process.env['XDG_DATA_HOME'] = path.join(sandbox, 'data');
process.env['XDG_CONFIG_HOME'] = path.join(sandbox, 'config');
process.env['XDG_CACHE_HOME'] = path.join(sandbox, 'cache');
process.env['WEBVIEW2_USER_DATA_FOLDER'] = path.join(sandbox, 'webview2');

// App folders of the e2e build and the answers of the folder dialog
// (WebDriver cannot operate native dialogs; see ADR 0008).
process.env['PW_E2E_APP_DIRS'] = appDirs;
process.env['PW_E2E_PICK_FOLDERS'] = [pickedFolders.local, pickedFolders.moveTarget].join(path.delimiter);
process.env['PW_E2E_PICK_FILES'] = Object.values(pickedFiles).join(path.delimiter);
process.env['PW_E2E_PICK_MEDIA'] = Object.values(pickedMedia).join(path.delimiter);
process.env['PW_E2E_PICK_EXPORT'] = Object.values(pickedExport).join(path.delimiter);
// No automatic backup 30 s after each start: it would run in the middle of
// the measurements (ADR 0021). Backups are made by hand in export-backup.e2e.ts.
process.env['PW_E2E_NO_AUTO_BACKUP'] = '1';
// Web links are recorded there instead of starting a browser.
process.env['PW_E2E_OPENED_LINKS'] = openedLinks;

const specs = path.join(root, 'tests', 'e2e', 'specs');

export const config: WebdriverIO.Config = {
  runner: 'local',
  // Fixed order: each file builds on the library the previous one left. The
  // Phase 1 smoke test needs the start screen, so it runs before a library
  // exists; library.e2e.ts ends the app, restart.e2e.ts gets a new process.
  // So do session-quit.e2e.ts (window closed), crash.e2e.ts (process
  // killed) and reference-prepare.e2e.ts (window closed, the reference
  // library configured): the files after them check the next start.
  specs: [
    path.join(specs, 'smoke.e2e.ts'),
    path.join(specs, 'library.e2e.ts'),
    path.join(specs, 'restart.e2e.ts'),
    path.join(specs, 'workbench.e2e.ts'),
    path.join(specs, 'editor.e2e.ts'),
    path.join(specs, 'performance.e2e.ts'),
    path.join(specs, 'external.e2e.ts'),
    path.join(specs, 'documents.e2e.ts'),
    path.join(specs, 'structure.e2e.ts'),
    path.join(specs, 'media.e2e.ts'),
    path.join(specs, 'search.e2e.ts'),
    path.join(specs, 'session-quit.e2e.ts'),
    path.join(specs, 'session-restored.e2e.ts'),
    path.join(specs, 'crash.e2e.ts'),
    path.join(specs, 'crash-recovered.e2e.ts'),
    path.join(specs, 'export-backup.e2e.ts'),
    path.join(specs, 'reference-prepare.e2e.ts'),
    path.join(specs, 'reference.e2e.ts'),
  ],
  maxInstances: 1,
  capabilities: [
    {
      browserName: 'tauri',
      'tauri:options': { application },
    } as WebdriverIO.Capabilities,
  ],
  services: [['@wdio/tauri-service', { driverProvider: 'embedded', startTimeout: 120_000 }]],
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 120_000 },
  // No outputDir: worker and service logs go to the console, so CI logs show
  // why a worker failed to start.
  logLevel: 'warn',
  waitforTimeout: 20_000,
};
