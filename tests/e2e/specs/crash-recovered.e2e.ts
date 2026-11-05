/**
 * Stage 2, "Absturz" (ADR 0016, decision 9), second half: after the hard
 * kill the next start offers the draft. Restored, the document holds
 * everything typed until at most 2 s before the kill (section 13).
 *
 * Runs in a new app process after crash.e2e.ts.
 */
import fs from 'node:fs';
import { $, browser, expect } from '@wdio/globals';
import { activeDocumentId, activeEditor, documentFile, draftFile } from '../editing';
import { handover } from '../sandbox';
import { pinMainWindow } from '../session';

const MAX_LOSS_MS = 2_000;

describe('Recovery after a hard kill (stage 2)', () => {
  before(async () => {
    await pinMainWindow();
  });

  it('offers the draft at the next start; at most 2 s of typing are lost', async () => {
    const crash = JSON.parse(fs.readFileSync(handover, 'utf8')) as {
      id: string;
      planned: string;
      typed: number[];
      killedAt: number;
    };
    const entry = $(`[data-testid="recovery-dialog"] [data-draft="${crash.id}"]`);
    await entry.waitForDisplayed({ timeout: 60_000, timeoutMsg: 'the next start did not offer the draft' });
    await $(`[data-testid="recovery-dialog"] [data-draft="${crash.id}"] [data-choice="restore"]`).click();
    await $('[data-testid="recovery-dialog"]').waitForExist({ reverse: true });

    const file = documentFile(crash.id, 'md');
    await browser.waitUntil(() => fs.existsSync(file) && fs.readFileSync(file, 'utf8').trim() !== '', {
      timeout: 10_000,
      timeoutMsg: 'the draft was not saved to the document',
    });
    const restored = fs.readFileSync(file, 'utf8').trimEnd();
    expect(crash.planned.startsWith(restored)).toBe(true);
    // The first character that did not come back was typed at most 2 s
    // before the kill (a trailing space counts as lost).
    const kept = restored.length;
    const firstLost = crash.typed[kept];
    const lostMs = firstLost === undefined ? 0 : crash.killedAt - firstLost;
    console.log(
      `Kept ${String(kept)} of ${String(crash.typed.length)} typed characters; lost the last ${String(lostMs)} ms.`,
    );
    expect(lostMs).toBeLessThanOrEqual(MAX_LOSS_MS);

    expect(await activeDocumentId()).toBe(crash.id);
    expect((await (await activeEditor()).getText()).trim()).toBe(restored);
    expect(fs.existsSync(draftFile(crash.id))).toBe(false);
  });
});
