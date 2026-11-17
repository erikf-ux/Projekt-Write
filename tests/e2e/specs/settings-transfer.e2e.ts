/**
 * Phase 13, settings transfer (ADR 0027, decisions 4 and 5) in the real
 * app: export the settings, change one, import the file (preview, then
 * "Übernehmen"), reset the interface settings, and open a storage location.
 * The file dialogs answer from `PW_E2E_PICK_SETTINGS` (sandbox.ts), in
 * order; opened folders are recorded in `PW_E2E_OPENED_LINKS`.
 *
 * Runs after language.e2e.ts in the same library. At the end the exported
 * file is imported once more, so the files after this one find the settings
 * as they were.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import { appDirs, openedLinks, pickedSettings, settingsDir } from '../sandbox';
import { closeSettings, openSettings, pinMainWindow } from '../session';

const configDir = path.join(appDirs, 'config');
const backupsDir = path.join(configDir, 'settings-backups');

function readSettings(): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(path.join(configDir, 'settings.json'), 'utf8')) as Record<
    string,
    unknown
  >;
}

function readExport(): Record<string, Record<string, unknown>> {
  return JSON.parse(fs.readFileSync(pickedSettings.export, 'utf8')) as Record<
    string,
    Record<string, unknown>
  >;
}

function backups(): string[] {
  return fs.existsSync(backupsDir)
    ? fs.readdirSync(backupsDir).filter((name) => name.endsWith('.pwsettings'))
    : [];
}

/** Clicks "Einstellungen importieren …" and waits for the preview. */
async function importAndPreview(): Promise<void> {
  await $('[data-testid="settings-import"]').click();
  await $('[data-testid="settings-import-preview"]').waitForDisplayed({ timeoutMsg: 'no import preview' });
}

/** "Übernehmen" in the preview. */
async function applyImport(): Promise<void> {
  await $('[data-testid="settings-import-apply"]').click();
  await $('[data-testid="settings-import-preview"]').waitForExist({
    reverse: true,
    timeoutMsg: 'preview stays open',
  });
  await expect($('[data-testid="settings-advanced-message"]')).toBeDisplayed();
}

describe('Settings transfer (Phase 13)', () => {
  let exportedZoom = 1;

  before(async () => {
    fs.mkdirSync(settingsDir, { recursive: true });
    await pinMainWindow();
    await closeSettings();
  });

  after(async () => {
    await closeSettings();
  });

  it('exports the settings without the paths of this computer', async () => {
    await openSettings('advanced');
    await $('[data-testid="settings-export"]').click();
    await browser.waitUntil(() => fs.existsSync(pickedSettings.export), {
      timeoutMsg: 'no settings file written',
    });
    await expect($('[data-testid="settings-advanced-message"]')).toBeDisplayed();

    const exported = readExport();
    expect(exported['format']).toBe('projectwrite-settings');
    expect(exported['formatVersion']).toBe(1);
    expect(exported['app']).toBeDefined();
    expect(exported['library']).toBeDefined();
    expect(Object.keys(exported['app'] ?? {})).not.toContain('libraryPath');
    expect(Object.keys(exported['app'] ?? {})).not.toContain('window');
    expect(fs.readFileSync(pickedSettings.export, 'utf8')).not.toContain(appDirs);
    exportedZoom = Number(exported['app']?.['zoom']);
    expect(exportedZoom).toBe(readSettings()['zoom']);
    await closeSettings();
  });

  it('imports the file after a preview and restores the changed setting', async () => {
    // Change the zoom after the export.
    await $('#pw-explorer-list').click();
    await browser.keys(['Control', '=']);
    await browser.waitUntil(() => readSettings()['zoom'] !== exportedZoom, {
      timeoutMsg: 'zoom not changed',
    });
    const changedZoom = Number(readSettings()['zoom']);

    await openSettings('advanced');
    await importAndPreview();
    const row = $('[data-testid="settings-changes-app"] tr[data-key="zoom"]');
    await expect(row).toHaveText(expect.stringContaining(`${String(Math.round(changedZoom * 100))} %`));
    await expect(row).toHaveText(expect.stringContaining(`${String(Math.round(exportedZoom * 100))} %`));
    // Nothing changes before "Übernehmen".
    expect(readSettings()['zoom']).toBe(changedZoom);
    expect(backups()).toHaveLength(0);

    await applyImport();
    await browser.waitUntil(() => readSettings()['zoom'] === exportedZoom, {
      timeoutMsg: 'zoom not restored',
    });
    expect(backups()).toHaveLength(1);
    expect(typeof readSettings()['libraryPath']).toBe('string');
    await closeSettings();
    // The running app shows the restored zoom at once.
    if (exportedZoom === 1) {
      await expect($('[data-testid="status-zoom"]')).not.toBeExisting();
    } else {
      await expect($('[data-testid="status-zoom"]')).toHaveText(
        expect.stringContaining(String(Math.round(exportedZoom * 100))),
      );
    }
  });

  it('resets the interface settings to their defaults after a confirmation', async () => {
    await openSettings('general');
    const limit = $('[data-testid="limit-line-width"]');
    if (await limit.isSelected()) {
      await limit.click();
    }
    await browser.waitUntil(() => readSettings()['limitLineWidth'] === false, {
      timeoutMsg: 'line width not changed',
    });

    await openSettings('advanced');
    await $('[data-testid="settings-reset-app"]').click();
    await $('[data-testid="settings-reset-confirm"] [data-choice="reset"]').click();
    await browser.waitUntil(() => readSettings()['limitLineWidth'] === true, {
      timeoutMsg: 'the interface settings were not reset',
    });
    const settings = readSettings();
    expect(settings['theme']).toBe('system');
    expect(settings['zoom']).toBe(1);
    expect(settings['keybindings']).toEqual({});
    expect(typeof settings['libraryPath']).toBe('string');
    expect(backups()).toHaveLength(2);
    // The general section shows the reset value at once.
    await openSettings('general');
    await expect($('[data-testid="limit-line-width"]')).toBeSelected();
  });

  it('opens the configuration folder in the file manager', async () => {
    await openSettings('advanced');
    await expect($('[data-testid="location-config-path"]')).toHaveText(expect.stringContaining('config'));
    await $('[data-testid="location-config-open"]').click();
    await browser.waitUntil(
      () =>
        fs.existsSync(openedLinks) && fs.readFileSync(openedLinks, 'utf8').split('\n').includes(configDir),
      { timeoutMsg: 'the configuration folder was not opened' },
    );
  });

  it('brings back the exported settings for the files after this one', async () => {
    await openSettings('advanced');
    await importAndPreview();
    await applyImport();
    await browser.waitUntil(
      () => readSettings()['limitLineWidth'] === readExport()['app']?.['limitLineWidth'],
      {
        timeoutMsg: 'the exported settings did not come back',
      },
    );
    const settings = readSettings();
    for (const [key, value] of Object.entries(readExport()['app'] ?? {})) {
      if (key === 'languageCheck') {
        continue;
      }
      expect(settings[key]).toEqual(value);
    }
    expect(backups()).toHaveLength(3);
  });
});
