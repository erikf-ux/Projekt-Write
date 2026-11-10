/**
 * Phase 7, "Der Explorer bleibt mit der Referenzbibliothek flüssig" (ADR
 * 0017, decision 17), measured in the running app with the library that
 * reference-prepare.e2e.ts built. The targets:
 *
 * * the tree stands at most 1 s after the library begins to open (the app
 *   sets two performance marks);
 * * opening and closing a subnet and a tag filter show within 100 ms;
 * * scrolling through a large open tree keeps 95 % of the frames below
 *   33 ms (at least 30 frames per second, the target of the Mesh).
 *
 * The values are logged and reported (bericht-phase-7.md). Opening and
 * closing is checked against its target. Opening the library, the filter
 * and scrolling depend on the machine (F-28: met on one container, missed
 * on another): they are checked against limits that catch a regression,
 * until the targets are confirmed on the reference device.
 *
 * Phase 8 adds the gallery with the 5,000 media of the reference library
 * (section 17: "Galerie, virtualisiert, mit Vorschaubildern"): it shows its
 * first tiles, keeps only the visible rows in the DOM and scrolls; the
 * values are logged for the report.
 *
 * Runs in a new app process after reference-prepare.e2e.ts.
 */
import { $, browser, expect } from '@wdio/globals';
import { runFromPalette } from '../editing';
import { pinMainWindow } from '../session';

/** Targets (ADR 0017, decision 17). */
const REACTION_MS = 100;
/** Regression limits while the targets wait for the reference device. */
const READY_LIMIT_MS = 1_500;
const FILTER_LIMIT_MS = 200;
const FRAME_LIMIT_MS = 50;

function percentile(values: number[], share: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(share * sorted.length) - 1)] ?? 0;
}

function format(values: number[]): string {
  const sorted = [...values].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
  return `median ${median.toFixed(1)} ms, 95 % ${percentile(values, 0.95).toFixed(1)} ms, max ${(sorted[sorted.length - 1] ?? 0).toFixed(1)} ms`;
}

describe('Explorer with the reference library (Phase 7)', () => {
  before(async () => {
    await pinMainWindow();
    try {
      await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 30_000 });
      await $('[data-testid="explorer"] [data-subnet]').waitForExist({ timeout: 30_000 });
    } catch (error) {
      console.log(
        'DEBUG reference',
        JSON.stringify(
          await browser.execute(() => ({
            workbench: document.querySelector('[data-testid="workbench"]') !== null,
            explorer: document.querySelector('[data-testid="explorer"]')?.outerHTML.slice(0, 1500),
            text: document.body.innerText.slice(0, 1500),
            dialogs: [...document.querySelectorAll('[role="dialog"]')].map((d) =>
              d.textContent?.slice(0, 200),
            ),
          })),
          null,
          1,
        ),
      );
      throw error;
    }
  });

  it('shows the tree after the library begins to open (target 1 s)', async () => {
    const samples: number[] = [];
    for (let run = 0; run < 3; run += 1) {
      // A reload opens the library in the interface again.
      await browser.refresh();
      await $('[data-testid="explorer"] [data-subnet]').waitForExist({ timeout: 60_000 });
      await browser.waitUntil(() =>
        browser.execute(() => performance.getEntriesByName('pw:explorer-ready').length > 0),
      );
      samples.push(
        await browser.execute(() => {
          const opening = performance.getEntriesByName('pw:library-opening')[0];
          const ready = performance.getEntriesByName('pw:explorer-ready')[0];
          return (ready?.startTime ?? 0) - (opening?.startTime ?? 0);
        }),
      );
    }
    const counts = await browser.execute(() => ({
      subnets: document.querySelectorAll('[data-testid="explorer"] [data-subnet][aria-level="1"]').length,
      unassigned: document.querySelector('[data-group="unassigned"]')?.textContent ?? '',
    }));
    console.log(
      `Explorer of the reference library (${process.platform}): ready after ${samples.map((ms) => ms.toFixed(0)).join(', ')} ms; ${String(counts.subnets)} subnets on the top level shown; ${counts.unassigned}`,
    );
    expect(Math.max(...samples)).toBeLessThanOrEqual(READY_LIMIT_MS);
  });

  it('opens and closes subnets within 100 ms and filters by a tag (target 100 ms)', async () => {
    const toggles = await browser.execute(async () => {
      const frame = () =>
        new Promise<void>((resolve) => {
          requestAnimationFrame(() => {
            resolve();
          });
        });
      const times: number[] = [];
      for (let round = 0; round < 10; round += 1) {
        const rows = [
          ...document.querySelectorAll<HTMLElement>('[data-testid="explorer"] [data-subnet][aria-level="1"]'),
        ];
        const row = rows[round % rows.length];
        if (!row) {
          break;
        }
        for (let click = 0; click < 2; click += 1) {
          const start = performance.now();
          row.click();
          await frame();
          await frame();
          times.push(performance.now() - start);
        }
      }
      return times;
    });

    await $('[data-testid="explorer-filter-toggle"]').click();
    await $('[data-testid="explorer-filter-tags"]').click();
    await $('[data-testid="explorer-filter-tag-list"] [data-tag-choice]').waitForExist();
    const filter = await browser.execute(async () => {
      const frame = () =>
        new Promise<void>((resolve) => {
          requestAnimationFrame(() => {
            resolve();
          });
        });
      const times: number[] = [];
      for (let round = 0; round < 5; round += 1) {
        const choice = document.querySelectorAll<HTMLElement>('[data-tag-choice]')[round];
        if (!choice) {
          break;
        }
        const start = performance.now();
        choice.click();
        await frame();
        await frame();
        times.push(performance.now() - start);
        // Off again, for the next tag.
        document.querySelector<HTMLElement>('[data-testid="explorer-filter-clear"]')?.click();
        await frame();
      }
      return times;
    });
    console.log(`Explorer: open or close a subnet ${format(toggles)}; tag filter ${format(filter)}`);
    expect(toggles.length).toBeGreaterThanOrEqual(10);
    expect(filter.length).toBe(5);
    expect(percentile(toggles, 0.95)).toBeLessThanOrEqual(REACTION_MS);
    expect(Math.max(...filter)).toBeLessThanOrEqual(FILTER_LIMIT_MS);
  });

  it('scrolls through a large open tree (target 30 frames per second)', async () => {
    // A title filter opens every subnet with a match: a long tree.
    const result = await browser.execute(async () => {
      const frame = () =>
        new Promise<number>((resolve) => {
          requestAnimationFrame((time) => {
            resolve(time);
          });
        });
      const input = document.querySelector<HTMLInputElement>('[data-testid="explorer-filter-text"]');
      if (!input) {
        throw new Error('no filter field');
      }
      // Typed the way the WebDriver types (see editing.ts).
      input.focus();
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
      document.execCommand('insertText', false, 'Kapitel');
      await frame();
      await frame();
      const list = document.getElementById('pw-explorer-list');
      if (!list) {
        throw new Error('no Explorer list');
      }
      const height = list.scrollHeight;
      // The machine's own pace first: frames without any work.
      const idle: number[] = [];
      let previous = await frame();
      for (let step = 0; step < 90; step += 1) {
        const now = await frame();
        idle.push(now - previous);
        previous = now;
      }
      // Fast scrolling, 80 px a frame (about 4,800 px/s), and a hard fling at 240 px.
      const scrolled = async (pixels: number) => {
        const intervals: number[] = [];
        let last = await frame();
        for (let step = 0; step < 180; step += 1) {
          list.scrollTop = (list.scrollTop + pixels) % Math.max(1, height - list.clientHeight);
          const now = await frame();
          intervals.push(now - last);
          last = now;
        }
        return intervals;
      };
      const deltas = await scrolled(80);
      const fling = await scrolled(240);
      // The floor of this machine for scrolling: a plain list of the same
      // size and place, without React, scrolled the same way.
      const plain = document.createElement('div');
      const rect = list.getBoundingClientRect();
      plain.style.cssText = `position:fixed;left:${String(rect.left)}px;top:${String(rect.top)}px;width:${String(rect.width)}px;height:${String(rect.height)}px;overflow-y:auto;z-index:99;background:var(--pw-color-sidebar)`;
      const inner = document.createElement('div');
      inner.style.cssText = `position:relative;height:${String(height)}px`;
      for (let row = 0; row < Math.round(height / 26); row += 1) {
        const item = document.createElement('div');
        item.className = 'text-ui';
        item.style.cssText = `position:absolute;left:0;right:0;top:${String(row * 26)}px;height:26px;padding-left:28px`;
        item.textContent = `Kapitel ${String(row)}`;
        inner.append(item);
      }
      plain.append(inner);
      document.body.append(plain);
      const floor: number[] = [];
      let before = await frame();
      for (let step = 0; step < 180; step += 1) {
        plain.scrollTop = (plain.scrollTop + 240) % Math.max(1, height - plain.clientHeight);
        const now = await frame();
        floor.push(now - before);
        before = now;
      }
      plain.remove();
      return { deltas, fling, idle, floor, rows: Math.round(height / 26) };
    });
    console.log(
      `Explorer: scrolling through ${String(result.rows)} rows at 80 px a frame ${format(result.deltas)}; at 240 px ${format(result.fling)}; without work ${format(result.idle)}; a plain list without React at 240 px ${format(result.floor)}`,
    );
    expect(result.rows).toBeGreaterThan(500);
    expect(percentile(result.deltas, 0.95)).toBeLessThanOrEqual(FRAME_LIMIT_MS);
  });
});

describe('Gallery with the reference library (Phase 8)', () => {
  it('shows 5,000 media virtualised and scrolls through them', async () => {
    const started = await browser.execute(() => performance.now());
    await runFromPalette('media.gallery');
    await $('[data-testid="gallery-list"] [data-media]').waitForDisplayed({ timeout: 30_000 });
    const shown = await browser.execute(() => performance.now());
    const result = await browser.execute(async () => {
      const list = document.querySelector<HTMLElement>('[data-testid="gallery-list"]');
      if (!list) {
        throw new Error('no gallery');
      }
      const frame = () =>
        new Promise<number>((resolve) => {
          requestAnimationFrame(() => {
            resolve(performance.now());
          });
        });
      const tilesBefore = list.querySelectorAll('[data-media]').length;
      const deltas: number[] = [];
      let before = await frame();
      for (let step = 0; step < 120; step += 1) {
        list.scrollTop += 240;
        const now = await frame();
        deltas.push(now - before);
        before = now;
      }
      return {
        tilesBefore,
        tilesAfter: list.querySelectorAll('[data-media]').length,
        height: list.scrollHeight,
        deltas,
      };
    });
    console.log(
      `Gallery of the reference library: first tiles after ${(shown - started).toFixed(0)} ms (palette included); ${String(result.tilesBefore)} tiles in the DOM, ${String(result.tilesAfter)} after scrolling; scrolling at 240 px a frame ${format(result.deltas)}`,
    );
    // Virtualised: a screenful of tiles, not 5,000.
    expect(result.tilesBefore).toBeGreaterThan(0);
    expect(result.tilesBefore).toBeLessThan(400);
    expect(result.tilesAfter).toBeLessThan(400);
    expect(result.height).toBeGreaterThan(20_000);
    expect(percentile(result.deltas, 0.95)).toBeLessThanOrEqual(FRAME_LIMIT_MS);
  });
});
