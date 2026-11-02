/**
 * End-to-end tests against the real app binary (debug build with the `e2e`
 * feature, see `npm run build:e2e`). The embedded WebDriver server of
 * `tauri-plugin-wdio-webdriver` works on Windows, macOS and Linux and needs no
 * `withGlobalTauri` (see ADR 0003).
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const binary = process.platform === 'win32' ? 'project-write.exe' : 'project-write';
// PW_E2E_APP lets experiments (for example ADR 0005) test another binary.
const application = process.env['PW_E2E_APP'] ?? path.join(root, 'src-tauri', 'target', 'debug', binary);

if (!fs.existsSync(application)) {
  throw new Error(`App binary not found at ${application}. Run "npm run build:e2e" first.`);
}

// Keep everything the webview writes (caches, local storage) inside the
// project folder (section 2: tests never write outside it).
const sandbox = path.join(root, 'tests', 'e2e', '.tmp');
fs.rmSync(sandbox, { recursive: true, force: true });
for (const dir of ['data', 'config', 'cache', 'webview2']) {
  fs.mkdirSync(path.join(sandbox, dir), { recursive: true });
}
process.env['XDG_DATA_HOME'] = path.join(sandbox, 'data');
process.env['XDG_CONFIG_HOME'] = path.join(sandbox, 'config');
process.env['XDG_CACHE_HOME'] = path.join(sandbox, 'cache');
process.env['WEBVIEW2_USER_DATA_FOLDER'] = path.join(sandbox, 'webview2');

export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: [path.join(root, 'tests', 'e2e', 'specs', '**', '*.e2e.ts')],
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
  logLevel: 'warn',
  outputDir: path.join(root, 'tests', 'e2e', 'logs'),
  waitforTimeout: 20_000,
};
