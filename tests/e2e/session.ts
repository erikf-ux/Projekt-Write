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

/** Closes the settings dialog with Escape. */
export async function closeSettings(): Promise<void> {
  const dialog = $('[data-testid="settings-dialog"]');
  if (await dialog.isExisting()) {
    await browser.keys('Escape');
    await dialog.waitForExist({ reverse: true, timeout: 20_000 });
  }
}
