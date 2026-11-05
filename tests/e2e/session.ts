/**
 * Session helpers for the end-to-end tests.
 */
import { $, browser } from '@wdio/globals';

/**
 * Pins the WebDriver session to the main window.
 *
 * `@wdio/tauri-service` otherwise asks the app for its window states before
 * every element command through `window.__TAURI__`, which this app does not
 * expose (`withGlobalTauri: false`, ADR 0003); each attempt waits 5 s. An
 * explicit window switch turns that lookup off.
 */
export async function pinMainWindow(): Promise<void> {
  const handle = await browser.getWindowHandle();
  await browser.switchToWindow(handle);
}

/** Opens the settings dialog on one of its sections (from the toolbar). */
export async function openSettings(section: 'general' | 'shortcuts' | 'library'): Promise<void> {
  const dialog = $('[data-testid="settings-dialog"]');
  if (!(await dialog.isExisting())) {
    await $('[data-testid="toolbar"] [data-command="settings.open"]').click();
    await dialog.waitForDisplayed({ timeout: 20_000 });
  }
  await $(`[data-testid="settings-tab-${section}"]`).click();
}

/**
 * Closes the settings dialog with Escape. A shortcut still being recorded
 * takes the first Escape, so it tries a few times: one failed test must not
 * leave the dialog open for every file after it.
 */
export async function closeSettings(): Promise<void> {
  const dialog = $('[data-testid="settings-dialog"]');
  for (let attempt = 0; attempt < 3 && (await dialog.isExisting()); attempt += 1) {
    await browser.keys('Escape');
    await dialog.waitForExist({ reverse: true, timeout: 5_000 }).catch(() => undefined);
  }
  if (await dialog.isExisting()) {
    throw new Error('the settings dialog stays open');
  }
}

/**
 * After the app ended (window closed or process killed): the runner's last
 * `deleteSession` would fail, and the next spec file may only start once
 * the embedded WebDriver no longer answers (then the test service starts
 * the app again; CI run 77).
 */
export async function waitUntilAppEnded(): Promise<void> {
  const overwrite = browser.overwriteCommand as (name: string, command: () => Promise<void>) => void;
  overwrite('deleteSession', async () => {
    // The session ended with the app.
  });
  const status = `http://${browser.options.hostname ?? '127.0.0.1'}:${String(browser.options.port ?? 4445)}/status`;
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      await fetch(status, { signal: AbortSignal.timeout(1_000) });
      await new Promise((resolve) => setTimeout(resolve, 200));
    } catch (error) {
      // A refused connection means the server is gone; a timeout, that it is ending.
      if (!(error instanceof DOMException && error.name === 'TimeoutError')) {
        return;
      }
    }
  }
  throw new Error('the app did not end');
}
