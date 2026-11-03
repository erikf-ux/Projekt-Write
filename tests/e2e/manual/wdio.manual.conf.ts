/**
 * Configuration for the manual dialog check: the normal E2E configuration
 * without scripted dialog answers, so the app shows the system dialog.
 */
import path from 'node:path';
import { config as base } from '../wdio.conf';
import { root } from '../sandbox';

process.env['PW_E2E_PICK_FOLDERS'] = '';
Reflect.deleteProperty(process.env, 'PW_E2E_PICK_FOLDERS');

export const config: WebdriverIO.Config = {
  ...base,
  specs: [path.join(root, 'tests', 'e2e', 'manual', 'dialog.manual.ts')],
};
