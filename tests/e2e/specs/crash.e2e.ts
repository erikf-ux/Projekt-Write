/**
 * Stage 2, "Absturz" (ADR 0016, decisions 8 and 9), first half: the app is
 * killed hard while the user types about ten characters a second. Autosave
 * waits for a pause of 1 s and never comes; only the draft in `recovery/`,
 * written at most a second after an input, keeps the text.
 * crash-recovered.e2e.ts checks the next start.
 *
 * Runs after session-restored.e2e.ts; kills the app (the test service
 * starts it again for the next file).
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { browser, expect } from '@wdio/globals';
import { activeEditor, draftFile, newDocument } from '../editing';
import { application, handover } from '../sandbox';
import { pinMainWindow, waitUntilAppEnded } from '../session';

const SENTENCE = 'Ein Satz nach dem anderen entsteht beim Tippen ohne Pause ';
const TYPING_MS = 5_000;

/** Ends the app the way a crash or power cut would: no clean-up runs. */
function killApp() {
  if (process.platform === 'win32') {
    execFileSync('taskkill', ['/F', '/T', '/IM', path.basename(application)], { stdio: 'ignore' });
    return;
  }
  const processes = execFileSync('ps', ['-eo', 'pid=,args='], { encoding: 'utf8' })
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '');
  const pids = processes
    .filter((line) => line.slice(line.indexOf(' ') + 1).startsWith(application))
    .map((line) => Number(line.slice(0, line.indexOf(' '))));
  if (pids.length === 0) {
    throw new Error('the app is not running');
  }
  for (const pid of pids) {
    process.kill(pid, 'SIGKILL');
  }
}

describe('Hard kill while typing (stage 2)', () => {
  before(async () => {
    await pinMainWindow();
  });

  it('leaves a draft that is at most a second old', async () => {
    const id = await newDocument();
    await (await activeEditor()).click();
    await browser.waitUntil(() =>
      browser.execute(() => document.activeElement?.getAttribute('data-testid') === 'editor'),
    );
    const planned = SENTENCE.repeat(4);
    // Types in the page, one character every 100 ms, and notes when each one
    // was typed; the WebDriver only starts and reads it.
    await browser.execute((text: string) => {
      const typed: number[] = [];
      (window as unknown as { pwTyped: number[] }).pwTyped = typed;
      const timer = setInterval(() => {
        const character = text[typed.length];
        if (character === undefined) {
          clearInterval(timer);
          return;
        }
        const target = document.activeElement ?? document.body;
        const options = { key: character, bubbles: true, cancelable: true, composed: true };
        if (target.dispatchEvent(new KeyboardEvent('keydown', options))) {
          // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
          document.execCommand('insertText', false, character);
        }
        target.dispatchEvent(new KeyboardEvent('keyup', options));
        typed.push(Date.now());
      }, 100);
    }, planned);
    await browser.pause(TYPING_MS);
    const typed = await browser.execute(() => [...(window as unknown as { pwTyped: number[] }).pwTyped]);
    const killedAt = Date.now();
    killApp();
    await waitUntilAppEnded();

    expect(typed.length).toBeGreaterThan(20);
    expect(fs.existsSync(draftFile(id))).toBe(true);
    const draft = JSON.parse(fs.readFileSync(draftFile(id), 'utf8')) as { text: string };
    expect(planned.startsWith(draft.text.trimEnd())).toBe(true);
    fs.writeFileSync(handover, JSON.stringify({ id, planned, typed, killedAt }));
  });
});
