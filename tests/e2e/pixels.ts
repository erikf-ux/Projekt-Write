/**
 * Measuring the rendered window (ADR 0003): the webviews of the CI machines
 * do not always report styles after a runtime change, the pixels on screen
 * are what counts.
 */
import { createHash } from 'node:crypto';
import { browser } from '@wdio/globals';
import { themes, type ThemeName } from '../../src/modules/themes/tokens';
import { decodePng, pixelHex, type Image } from './png';

/**
 * Waits until the window shows a settled frame and returns it as an image.
 *
 * The CI windows on Windows and macOS render asynchronously and throttled:
 * getComputedStyle lagged behind changes there, and on macOS even a single
 * screenshot could still show the previous state (CI runs 15–18). So the test
 * takes screenshots until two in a row are pixel-identical and, if a theme is
 * given, its surface colour is on screen – then measures that frame. In a
 * visible window this settles within a frame; the checks themselves are as
 * strict as before.
 */
export async function settledFrame(theme?: ThemeName): Promise<{ image: Image; scale: number }> {
  const viewportWidth = await browser.execute(() => window.innerWidth);
  let previous = '';
  let latest: Image | undefined;
  for (let attempt = 0; attempt < 15; attempt++) {
    const image = decodePng(Buffer.from(await browser.takeScreenshot(), 'base64'));
    const scale = image.width / viewportWidth;
    const fingerprint = createHash('sha256').update(image.pixels).digest('hex');
    const themeVisible =
      theme === undefined ||
      colourDistance(pixelHex(image, Math.round(6 * scale), Math.round(6 * scale)), themes[theme].surface) <=
        PIXEL_TOLERANCE;
    if (fingerprint === previous && themeVisible) {
      return { image, scale };
    }
    previous = fingerprint;
    latest = image;
    await browser.pause(300);
  }
  // Not settled: measure the last frame; the assertions report what is wrong.
  const image = latest ?? decodePng(Buffer.from(await browser.takeScreenshot(), 'base64'));
  return { image, scale: image.width / viewportWidth };
}

/** Distance between two `#RRGGBB` colours (largest channel difference). */
export function colourDistance(a: string, b: string): number {
  const channels = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const [x, y] = [channels(a), channels(b)];
  return Math.max(...x.map((value, i) => Math.abs(value - (y[i] ?? 0))));
}

/** Rendering may convert colour spaces slightly (macOS); allow small deviations. */
export const PIXEL_TOLERANCE = 8;

/**
 * Accent-coloured pixels that appear around `rect` (CSS pixels, widened by
 * 6 px) between two frames: the focus indicator drawn when the element got
 * the focus.
 */
export function newAccentPixels(
  before: Image,
  after: Image,
  scale: number,
  rect: { left: number; top: number; width: number; height: number },
  accent: string,
): number {
  let count = 0;
  const x0 = Math.max(0, Math.floor((rect.left - 6) * scale));
  const y0 = Math.max(0, Math.floor((rect.top - 6) * scale));
  const x1 = Math.min(after.width, Math.ceil((rect.left + rect.width + 6) * scale));
  const y1 = Math.min(after.height, Math.ceil((rect.top + rect.height + 6) * scale));
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const now = pixelHex(after, x, y);
      if (
        colourDistance(now, accent) <= PIXEL_TOLERANCE &&
        colourDistance(pixelHex(before, x, y), accent) > PIXEL_TOLERANCE
      ) {
        count++;
      }
    }
  }
  return count;
}
