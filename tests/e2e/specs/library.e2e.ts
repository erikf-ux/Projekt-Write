/**
 * Phase 2 in the running app (section 25): create a library, the sync folder
 * warning with rollback mode, moving a library (copy, check, switch).
 *
 * The folder dialog answers from PW_E2E_PICK_FOLDERS (see wdio.conf.ts):
 * first the local folder, then the move target inside the sync folder.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import { appDirs, pickedFolders } from '../sandbox';
import { pinMainWindow } from '../session';

async function libraryState() {
  return $('[data-testid="library"]').getAttribute('data-state');
}

async function waitForState(state: string) {
  const panel = $('[data-testid="library"]');
  await panel.waitUntil(async () => (await libraryState()) === state, {
    timeout: 60_000,
    timeoutMsg: `library panel did not reach state "${state}"`,
  });
}

function readJson(file: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(file, 'utf8')) as Record<string, unknown>;
}

describe('Project Write – Phase 2 library', () => {
  before(async () => {
    await pinMainWindow();
    await $('h1').waitForExist({ timeout: 60_000 });
    await waitForState('closed');
  });

  it('offers Dokumente/ProjectWrite and warns because it lies in a sync folder', async () => {
    const location = $('[data-testid="library-default-location"]');
    await expect(location).toHaveText(
      expect.stringContaining(path.join('OneDrive', 'documents', 'ProjectWrite')),
    );

    await $('[data-testid="library-create-default"]').click();
    const prompt = $('[data-testid="library-prompt"]');
    await prompt.waitForDisplayed();
    await expect(prompt).toHaveAttribute('data-kind', 'confirmLocation');
    await expect(prompt).toHaveText(expect.stringContaining('OneDrive'));
    // Nothing was created yet.
    expect(fs.existsSync(path.join(appDirs, 'documents', 'ProjectWrite'))).toBe(false);
  });

  it('creates the library at another, local location instead', async () => {
    await $('[data-testid="library-prompt-other"]').click();
    await waitForState('open');
    await expect($('[data-testid="library-name"]')).toHaveText('Bibliothek');
    await expect($('[data-testid="library-path"]')).toHaveText(
      expect.stringContaining(path.join('Lokal', 'Bibliothek')),
    );
    await expect($('[data-testid="library-journal"]')).toHaveText(expect.stringMatching(/^WAL/));
    expect(fs.existsSync(path.join(pickedFolders.local, 'library.json'))).toBe(true);
    expect(fs.existsSync(path.join(pickedFolders.local, 'database', 'projectwrite.db'))).toBe(true);
    // The location is remembered in the app settings.
    const settings = readJson(path.join(appDirs, 'config', 'settings.json'));
    expect(String(settings['libraryPath'])).toContain(path.join('Lokal', 'Bibliothek'));
    expect(fs.existsSync(path.join(appDirs, 'documents', 'ProjectWrite'))).toBe(false);
  });

  it('moves the library into the sync folder after a confirmation and uses rollback mode there', async () => {
    await $('[data-testid="library-move"]').click();
    const prompt = $('[data-testid="library-prompt"]');
    await prompt.waitForDisplayed();
    await expect(prompt).toHaveAttribute('data-kind', 'confirmLocation');
    await $('[data-testid="library-prompt-confirm"]').click();

    await waitForState('open');
    await expect($('[data-testid="library-path"]')).toHaveText(expect.stringContaining('Umzug'));
    await expect($('[data-testid="library-storage"]')).toHaveText(expect.stringContaining('OneDrive'));
    await expect($('[data-testid="library-journal"]')).toHaveText(expect.stringMatching(/^Rollback/));
    // Confirmed while moving: no notice for the new location.
    await expect($('[data-testid="library-location-warning"]')).not.toBeExisting();
    await expect($('[data-testid="library-move-notice"]')).toHaveText(
      expect.stringContaining(path.join('Lokal', 'Bibliothek')),
    );

    // Copied and complete; the old folder stays and knows where the library went.
    expect(fs.existsSync(path.join(pickedFolders.moveTarget, 'library.json'))).toBe(true);
    expect(fs.existsSync(path.join(pickedFolders.moveTarget, 'database', 'projectwrite.db'))).toBe(true);
    // Rollback journal: no WAL file next to the database.
    expect(fs.existsSync(path.join(pickedFolders.moveTarget, 'database', 'projectwrite.db-wal'))).toBe(false);
    const old = readJson(path.join(pickedFolders.local, 'library.json'));
    expect((old['movedTo'] as { path: string }).path).toContain('Umzug');
    const settings = readJson(path.join(appDirs, 'config', 'settings.json'));
    expect(String(settings['libraryPath'])).toContain('Umzug');
  });

  it('closes the library cleanly when the window is closed', async () => {
    // The app ends with its last window; the next spec file gets a new
    // process (the test service restarts it).
    // WebdriverIO ends the session when the last window goes away and reports
    // that as an error; here it is the expected outcome.
    await browser.closeWindow().catch((error: unknown) => {
      if (!String(error).includes('All window handles were removed')) {
        throw error;
      }
    });
    const log = path.join(appDirs, 'logs', 'project-write.log');
    const deadline = Date.now() + 30_000;
    let text = '';
    while (Date.now() < deadline) {
      text = fs.readFileSync(log, 'utf8');
      if (text.includes('Project Write stopped')) {
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    expect(text).toContain('Project Write stopped');
    // The library was closed before the process ended (lock released,
    // database closed).
    const lines = text.trimEnd().split('\n');
    const closed = lines.findLastIndex((line) => / library \S+ closed$/.test(line));
    const stopped = lines.findLastIndex((line) => line.includes('Project Write stopped'));
    expect(closed).toBeGreaterThan(-1);
    expect(closed).toBeLessThan(stopped);
  });
});
