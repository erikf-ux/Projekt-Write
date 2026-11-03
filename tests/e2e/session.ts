/**
 * Session helpers for the end-to-end tests.
 */
import { browser } from '@wdio/globals';

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
