/**
 * Phase 2 in the running app (section 25): the next start opens the library
 * again, a second start of the app hands over to the running one ("Eine
 * zweite Instanz kann dieselbe Bibliothek nicht öffnen"), and the old copy of
 * a moved library opens only after a confirmation.
 *
 * Runs after library.e2e.ts, in a new app process (library.e2e.ts closes the
 * window; the test service starts the app again).
 */
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, expect } from '@wdio/globals';
import { appDirs, application, pickedFolders } from '../sandbox';
import { pinMainWindow } from '../session';

async function waitForOpenLibrary() {
  const panel = $('[data-testid="library"]');
  await panel.waitUntil(async () => (await panel.getAttribute('data-state')) === 'open', {
    timeout: 60_000,
    timeoutMsg: 'the remembered library was not opened',
  });
}

/** Starts the app binary a second time; resolves with its exit code. */
function startSecondInstance(timeoutMs: number): Promise<number | null> {
  return new Promise((resolve, reject) => {
    const child = spawn(application, [], { env: process.env, stdio: 'ignore' });
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`the second instance did not end within ${String(timeoutMs)} ms`));
    }, timeoutMs);
    child.on('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.on('exit', (code) => {
      clearTimeout(timer);
      resolve(code);
    });
  });
}

describe('Project Write – Phase 2 restart and second instance', () => {
  before(async () => {
    await pinMainWindow();
    await $('h1').waitForExist({ timeout: 60_000 });
    await waitForOpenLibrary();
  });

  it('opens the library of the last session at start', async () => {
    // A new process: the log shows a second start.
    const log = fs.readFileSync(path.join(appDirs, 'logs', 'project-write.log'), 'utf8');
    expect(log.match(/Project Write \S+ starting/g)?.length).toBeGreaterThanOrEqual(2);
    await expect($('[data-testid="library-path"]')).toHaveText(
      expect.stringContaining(path.basename(pickedFolders.moveTarget)),
    );
    await expect($('[data-testid="library-journal"]')).toHaveText(expect.stringMatching(/^Rollback/));
  });

  it('hands a second start over to the running instance', async () => {
    // tauri-plugin-single-instance: the second process passes its arguments
    // to this one and ends with exit code 0 (Linux needs a D-Bus session).
    const code = await startSecondInstance(60_000);
    expect(code).toBe(0);
    // This instance keeps its library and still answers.
    await expect($('[data-testid="library-path"]')).toHaveText(
      expect.stringContaining(path.basename(pickedFolders.moveTarget)),
    );
  });

  it('opens the old copy of a moved library only after a confirmation', async () => {
    // A new process: the dialog answers from the start of the list, i.e. the
    // old, moved folder.
    const buttons = await $$('[data-testid="library"] button').getElements();
    let openOther: WebdriverIO.Element | undefined;
    for (const button of buttons) {
      const text = await button.getText();
      if (text.includes('Andere Bibliothek öffnen') || text.includes('Open another library')) {
        openOther = button;
      }
    }
    if (!openOther) {
      throw new Error('button "open another library" not found');
    }
    await openOther.click();
    const prompt = $('[data-testid="library-prompt"]');
    await prompt.waitForDisplayed();
    await expect(prompt).toHaveAttribute('data-kind', 'movedLibrary');
    await expect(prompt).toHaveText(expect.stringContaining('Umzug'));
    await $('[data-testid="library-prompt-cancel"]').click();
    await expect(prompt).not.toBeExisting();
    await expect($('[data-testid="library-path"]')).toHaveText(
      expect.stringContaining(path.basename(pickedFolders.moveTarget)),
    );
  });
});
