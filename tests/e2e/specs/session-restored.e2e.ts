/**
 * Stage 1, "Sitzung" (ADR 0016, decision 10), second half: after the
 * restart the tabs of the last session are open again, in their order,
 * with the active tab and each tab's selection.
 *
 * Runs in a new app process after session-quit.e2e.ts.
 */
import fs from 'node:fs';
import { $, $$, browser, expect } from '@wdio/globals';
import { activeDocumentId } from '../editing';
import { handover } from '../sandbox';
import { pinMainWindow } from '../session';

describe('Session after a restart (stage 1)', () => {
  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
  });

  it('opens the tabs of the last session again, with the active tab and the selection', async () => {
    const saved = JSON.parse(fs.readFileSync(handover, 'utf8')) as {
      first: string;
      second: string;
      library: string[];
      external: number;
    };
    await browser.waitUntil(async () => (await activeDocumentId()) === saved.second, {
      timeout: 60_000,
      timeoutMsg: 'the active tab of the last session is not active again',
    });
    const open: string[] = [];
    for (const tab of await $$('[data-testid="tabs"] [role="tab"]').getElements()) {
      open.push((await tab.getAttribute('data-tab')) ?? '');
    }
    expect(open.filter((id) => !id.startsWith('ext:'))).toEqual(saved.library);
    expect(open.filter((id) => id.startsWith('ext:'))).toHaveLength(saved.external);
    // A normal end leaves nothing to recover.
    expect(await $('[data-testid="recovery-dialog"]').isExisting()).toBe(false);

    // The selection of the first document ("Mitte") is back.
    await $(`[data-testid="tabs"] [data-tab="${saved.first}"]`).click();
    await browser.waitUntil(async () => (await activeDocumentId()) === saved.first);
    await expect($('[data-testid="status-words"]')).toHaveText('Wörter: 1 von 3');
  });
});
