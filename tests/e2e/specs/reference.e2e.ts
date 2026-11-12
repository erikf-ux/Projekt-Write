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
 * first tiles, keeps only the visible rows in the DOM and scrolls, once
 * through thumbnails it has not loaded yet and once more through the same
 * stretch; the values are logged for the report.
 *
 * Phase 9 measures the global search: from typing a query to its hits on
 * screen, against the target of section 6 (300 ms).
 *
 * Runs in a new app process after reference-prepare.e2e.ts.
 */
import { $, browser, expect } from '@wdio/globals';
import { runFromPalette } from '../editing';
import { measured } from '../measurements';
import { pinMainWindow } from '../session';

/** Targets (ADR 0017, decision 17). */
const REACTION_MS = 100;
/** Regression limits while the targets wait for the reference device. */
const READY_LIMIT_MS = 1_500;
const FILTER_LIMIT_MS = 200;
const FRAME_LIMIT_MS = 50;
/** Gallery, first pass through not yet loaded thumbnails (Phase 8). */
const GALLERY_FIRST_PASS_LIMIT_MS = 150;
/** Global search, first hits (section 6: 300 ms; Phase 9). */
const SEARCH_LIMIT_MS = 300;
/** The pause of the search field after the last keystroke (searchStore.ts). */
const SEARCH_DELAY_MS = 60;

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
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 120_000 });
    await $('[data-testid="explorer"] [data-subnet]').waitForExist({ timeout: 120_000 });
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
    measured(
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
      // 120 frames at 240 px: a fast fling through about 170 rows.
      const pass = async () => {
        const deltas: number[] = [];
        let before = await frame();
        for (let step = 0; step < 120; step += 1) {
          list.scrollTop += 240;
          const now = await frame();
          deltas.push(now - before);
          before = now;
        }
        return deltas;
      };
      const tilesBefore = list.querySelectorAll('[data-media]').length;
      // First pass: every thumbnail is loaded and decoded for the first time.
      const first = await pass();
      const tilesAfter = list.querySelectorAll('[data-media]').length;
      // The same stretch again, the thumbnails now in the webview's cache.
      list.scrollTop = 0;
      await new Promise((resolve) => setTimeout(resolve, 1_500));
      const again = await pass();
      return { tilesBefore, tilesAfter, height: list.scrollHeight, first, again };
    });
    measured(
      `Gallery of the reference library: first tiles after ${(shown - started).toFixed(0)} ms (palette included); ${String(result.tilesBefore)} tiles in the DOM, ${String(result.tilesAfter)} after scrolling; scrolling at 240 px a frame, first pass ${format(result.first)}; same stretch again ${format(result.again)}`,
    );
    // Virtualised: a screenful of tiles, not 5,000.
    expect(result.tilesBefore).toBeGreaterThan(0);
    expect(result.tilesBefore).toBeLessThan(400);
    expect(result.tilesAfter).toBeLessThan(400);
    expect(result.height).toBeGreaterThan(20_000);
    // The master prompt names no target for the gallery: the cached pass keeps the
    // Explorer's limit, the first pass (loading and decoding every
    // thumbnail) a limit that catches a regression (bericht-phase-8.md).
    expect(percentile(result.again, 0.95)).toBeLessThanOrEqual(FRAME_LIMIT_MS);
    expect(percentile(result.first, 0.95)).toBeLessThanOrEqual(GALLERY_FIRST_PASS_LIMIT_MS);
  });
});

describe('Search with the reference library (Phase 9)', () => {
  it('shows the first hits within 300 ms (section 6)', async () => {
    await browser.keys(['Control', 'Shift', 'f']);
    await $('[data-testid="search-field"]').waitForDisplayed();
    const queries = [
      'Krankenhaus',
      'haus',
      'Muller',
      'Mueller',
      'Strasse',
      'C++',
      'Kapitel',
      'e',
      'zuerich',
      'Literatur Quelle Zitat',
    ];
    const samples = await browser.execute(async (texts: string[]) => {
      const field = document.querySelector<HTMLInputElement>('[data-testid="search-field"]');
      if (!field) {
        throw new Error('no search field');
      }
      const shown = () => performance.getEntriesByName('pw:search-shown');
      const results: { query: string; ms: number; hits: number }[] = [];
      // Twice: the first round also warms SQLite's page cache.
      for (let round = 0; round < 2; round += 1) {
        for (const text of texts) {
          const before = shown().length;
          field.focus();
          field.select();
          const start = performance.now();
          // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
          document.execCommand('insertText', false, text);
          const until = start + 10_000;
          while (shown().length === before && performance.now() < until) {
            await new Promise((resolve) => setTimeout(resolve, 2));
          }
          const mark = shown()[before];
          if (round === 1) {
            results.push({
              query: text,
              ms: mark ? mark.startTime - start : Number.POSITIVE_INFINITY,
              hits: document.querySelectorAll('[data-testid="search-results"] [data-hit]').length,
            });
          }
        }
      }
      return results;
    }, queries);
    measured(
      `Search in the reference library (${process.platform}), from typing to hits on screen (${String(SEARCH_DELAY_MS)} ms pause included): ${samples
        .map((sample) => `${sample.query} ${sample.ms.toFixed(0)} ms (${String(sample.hits)})`)
        .join('; ')}`,
    );
    for (const sample of samples) {
      expect(sample.hits).toBeGreaterThan(0);
    }
    expect(Math.max(...samples.map((sample) => sample.ms))).toBeLessThanOrEqual(SEARCH_LIMIT_MS);
  });
});

/** Section 6: "Mesh beim Zoomen und Verschieben ≥ 30 Bilder pro Sekunde". */
const MESH_FRAME_TARGET_MS = 1000 / 30;

describe('Mesh with the reference library (Phase 11, load test)', () => {
  it('draws 1,000 nodes and zooms and pans them (target 30 frames per second)', async () => {
    // The measurement takes longer than the default 30 s for a script.
    await browser.setTimeout({ script: 180_000 });
    const started = await browser.execute(() => performance.now());
    await runFromPalette('mesh.open');
    await $('[data-testid="mesh"] .react-flow__node').waitForDisplayed({ timeout: 30_000 });
    const opened = await browser.execute(() => performance.now());
    // The cluster of elements without a subnet: 2,788 elements, so the view
    // reaches the limit of 1,000 drawn nodes.
    await $('[data-mesh-node="unassigned"] [data-testid="mesh-expand"]').click();
    await $('[data-testid="mesh-hidden"]').waitForDisplayed({ timeout: 30_000 });
    // Fitted into view: React Flow draws only what is in view.
    await $('[data-testid="mesh-canvas"]').click();
    await browser.keys('0');
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => document.querySelectorAll('[data-testid="mesh"] .react-flow__node').length,
        )) >= 900,
      { timeout: 30_000, timeoutMsg: 'the opened cluster was not drawn' },
    );
    const expanded = await browser.execute(() => performance.now());
    const result = await browser.execute(async () => {
      const pane = document.querySelector<HTMLElement>('[data-testid="mesh"] .react-flow__pane');
      const viewport = document.querySelector<HTMLElement>('[data-testid="mesh"] .react-flow__viewport');
      if (!pane || !viewport) {
        throw new Error('no Mesh canvas');
      }
      const frame = () =>
        new Promise<number>((resolve) => {
          requestAnimationFrame(() => {
            resolve(performance.now());
          });
        });
      const box = pane.getBoundingClientRect();
      const cx = box.left + box.width / 2;
      const cy = box.top + box.height / 2;
      const settle = async () => {
        for (let i = 0; i < 10; i += 1) {
          await frame();
        }
      };
      await settle();
      const nodes = document.querySelectorAll('[data-testid="mesh"] .react-flow__node').length;
      const edges = document.querySelectorAll('[data-testid="mesh"] .react-flow__edge').length;
      // Zoom: one wheel step a frame, in and out again.
      const zoom: number[] = [];
      let before = await frame();
      for (let step = 0; step < 120; step += 1) {
        pane.dispatchEvent(
          new WheelEvent('wheel', {
            deltaY: step < 60 ? -60 : 60,
            clientX: cx,
            clientY: cy,
            bubbles: true,
            cancelable: true,
          }),
        );
        const now = await frame();
        zoom.push(now - before);
        before = now;
      }
      const zoomed = viewport.style.transform;
      await settle();
      // Pan: drag the canvas 12 px a frame, there and back.
      const pan: number[] = [];
      pane.dispatchEvent(
        new MouseEvent('mousedown', {
          clientX: cx,
          clientY: cy,
          button: 0,
          buttons: 1,
          bubbles: true,
          view: window,
        }),
      );
      before = await frame();
      for (let step = 0; step < 120; step += 1) {
        const offset = step < 60 ? step * 12 : (120 - step) * 12;
        window.dispatchEvent(
          new MouseEvent('mousemove', {
            clientX: cx + offset,
            clientY: cy + offset / 2,
            buttons: 1,
            bubbles: true,
            view: window,
          }),
        );
        const now = await frame();
        pan.push(now - before);
        before = now;
      }
      window.dispatchEvent(
        new MouseEvent('mouseup', { clientX: cx, clientY: cy, bubbles: true, view: window }),
      );
      const panned = viewport.style.transform;
      await settle();
      // What costs: the same pan without the edges, and without the nodes.
      const panWithout = async (selector: string) => {
        const layer = document.querySelector<HTMLElement>(`[data-testid="mesh"] ${selector}`);
        if (layer) {
          layer.style.display = 'none';
        }
        await settle();
        const deltas: number[] = [];
        pane.dispatchEvent(
          new MouseEvent('mousedown', {
            clientX: cx,
            clientY: cy,
            button: 0,
            buttons: 1,
            bubbles: true,
            view: window,
          }),
        );
        let last = await frame();
        for (let step = 0; step < 60; step += 1) {
          window.dispatchEvent(
            new MouseEvent('mousemove', {
              clientX: cx + step * 6,
              clientY: cy,
              buttons: 1,
              bubbles: true,
              view: window,
            }),
          );
          const now = await frame();
          deltas.push(now - last);
          last = now;
        }
        window.dispatchEvent(
          new MouseEvent('mouseup', { clientX: cx, clientY: cy, bubbles: true, view: window }),
        );
        if (layer) {
          layer.style.display = '';
        }
        await settle();
        return deltas;
      };
      const noEdges = await panWithout('.react-flow__edges');
      const noNodes = await panWithout('.react-flow__nodes');
      // The same frames without work, for comparison.
      const idle: number[] = [];
      before = await frame();
      for (let step = 0; step < 60; step += 1) {
        const now = await frame();
        idle.push(now - before);
        before = now;
      }
      return { nodes, edges, zoom, pan, idle, zoomed, panned, noEdges, noNodes };
    });
    measured(
      `Mesh of the reference library (${process.platform}): open ${(opened - started).toFixed(0)} ms, cluster with 1,000 nodes ${(expanded - opened).toFixed(0)} ms; ${String(result.nodes)} nodes and ${String(result.edges)} edges in the DOM; zoom ${format(result.zoom)}; pan ${format(result.pan)}; pan without edges ${format(result.noEdges)}, without nodes ${format(result.noNodes)}; without work ${format(result.idle)}`,
    );
    expect(result.nodes).toBeGreaterThanOrEqual(900);
    // The canvas did move.
    expect(result.zoomed).not.toBe('');
    expect(result.panned).not.toBe(result.zoomed);
    // The load test of section 15 measures first; the renderer is decided
    // per ADR 0022 from these values on Windows and Linux. Until then the
    // target is reported, not enforced.
    measured(
      `Mesh target 30 frames per second (${MESH_FRAME_TARGET_MS.toFixed(1)} ms): zoom ${percentile(result.zoom, 0.95) <= MESH_FRAME_TARGET_MS ? 'met' : 'missed'}, pan ${percentile(result.pan, 0.95) <= MESH_FRAME_TARGET_MS ? 'met' : 'missed'}`,
    );
  });
});
