/**
 * Manual check, not part of `npm run test:e2e`: opens the real folder
 * dialog (no PW_E2E_PICK_FOLDERS) and takes a screenshot of the X display.
 * Run with tests/e2e/manual/wdio.manual.conf.ts under xvfb-run.
 */
import { execFileSync } from 'node:child_process';
import { $, $$, browser } from '@wdio/globals';
import { pinMainWindow } from '../session';

describe('manual: native folder dialog', () => {
  it('opens the folder dialog of the system', async () => {
    await pinMainWindow();
    await $('[data-testid="library"]').waitForExist({ timeout: 60_000 });
    const buttons = await $$('[data-testid="library"] button').getElements();
    for (const button of buttons) {
      const text = await button.getText();
      if (text.includes('Anderen Ort') || text.includes('another location')) {
        await button.click();
        break;
      }
    }
    await browser.pause(4000);
    execFileSync('import', ['-window', 'root', process.env['PW_SHOT'] ?? 'dialog.png']);
  });
});
