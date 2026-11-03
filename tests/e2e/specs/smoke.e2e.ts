/**
 * Phase 1 acceptance tests in the running app (section 25):
 * start, CSP, rejected file system access, contrast, keyboard focus.
 * Runs last; the start screen then shows the library of library.e2e.ts.
 */
import { browser, $, $$, expect } from '@wdio/globals';
import { contrastRatio, MIN_TEXT_CONTRAST } from '../../../src/modules/themes/contrast';
import { themes, type ThemeName } from '../../../src/modules/themes/tokens';
import { pixelHex, type Image } from '../png';
import { colourDistance, PIXEL_TOLERANCE, settledFrame } from '../pixels';
import { pinMainWindow } from '../session';

const READY_TEXTS = ['Alles bereit.', 'Everything is ready.'];

async function waitForChecks() {
  const summary = $('[data-testid="check-summary"]');
  await summary.waitUntil(async () => READY_TEXTS.includes(await summary.getText()), {
    timeoutMsg: 'system check did not report "ready"',
  });
}

/** Screen position (CSS pixels) inside the main surface and inside the system-check card. */
async function backgroundPoints() {
  return browser.execute(() => {
    const card = document.querySelector('[data-testid="system-check"]')?.getBoundingClientRect();
    return {
      surface: { x: 6, y: 6 },
      sidebar: card ? { x: card.left + 8, y: card.top + 8 } : { x: 0, y: 0 },
    };
  });
}

/** Selectors of every text element on the start screen. */
const TEXT_SELECTORS = [
  'h1',
  'header p',
  'h2',
  '[data-testid="library"] :is(p, dt, dd)',
  '[data-testid="check-summary"]',
  '[data-check] span',
  'button',
  'footer span',
];

/** Text boxes (CSS pixels) of every non-empty text element. */
async function textBoxes() {
  return browser.execute((selectors: string[]) => {
    return selectors.flatMap((selector) =>
      Array.from(document.querySelectorAll(selector))
        .map((element) => {
          const range = document.createRange();
          range.selectNodeContents(element);
          const rect = range.getBoundingClientRect();
          return {
            selector,
            text: element.textContent.trim(),
            x: rect.left,
            y: rect.top,
            width: rect.width,
            height: rect.height,
          };
        })
        .filter((box) => box.text !== '' && box.text !== '·' && box.width > 0 && box.height > 0),
    );
  }, TEXT_SELECTORS);
}

/**
 * Rendered contrast of one text box: background is the most frequent pixel in
 * the box, text colour the pixel with the highest contrast against it (glyph
 * cores reach the full text colour; checked at 1x and 2x scale).
 */
function renderedContrast(
  image: Image,
  scale: number,
  box: { x: number; y: number; width: number; height: number },
) {
  const counts = new Map<string, number>();
  const pixels: string[] = [];
  for (let y = Math.ceil(box.y * scale); y < Math.floor((box.y + box.height) * scale); y++) {
    for (let x = Math.ceil(box.x * scale); x < Math.floor((box.x + box.width) * scale); x++) {
      const pixel = pixelHex(image, x, y);
      pixels.push(pixel);
      counts.set(pixel, (counts.get(pixel) ?? 0) + 1);
    }
  }
  const background = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? '#000000';
  let text = background;
  let ratio = 1;
  for (const pixel of pixels) {
    const candidate = contrastRatio(pixel, background);
    if (candidate > ratio) {
      ratio = candidate;
      text = pixel;
    }
  }
  return { text, background, ratio };
}

/**
 * Asserts that `theme` is on screen and every text reaches 4.5:1 (section 6),
 * measured on the rendered pixels – what the user sees. Computed styles are
 * not used for colours: in the CI windows on macOS and Windows they lag behind
 * the rendered frame (docs/adr/0003).
 */
async function expectContrast(theme: ThemeName) {
  const { image, scale } = await settledFrame(theme);
  const problems: string[] = [];
  const active = await browser.execute(() => document.documentElement.dataset['theme'] ?? '');
  if (active !== theme) {
    problems.push(`data-theme is "${active}", expected "${theme}"`);
  }
  const points = await backgroundPoints();
  for (const [token, point] of Object.entries(points) as [
    'surface' | 'sidebar',
    { x: number; y: number },
  ][]) {
    const pixel = pixelHex(image, Math.round(point.x * scale), Math.round(point.y * scale));
    if (colourDistance(pixel, themes[theme][token]) > PIXEL_TOLERANCE) {
      problems.push(`rendered ${token} pixel ${pixel}, expected ${themes[theme][token]}`);
    }
  }
  const boxes = await textBoxes();
  if (boxes.length <= 10) {
    problems.push(`only ${String(boxes.length)} text elements found`);
  }
  for (const box of boxes) {
    const { text, background, ratio } = renderedContrast(image, scale, box);
    if (ratio < MIN_TEXT_CONTRAST) {
      problems.push(`${box.selector} "${box.text}": ${text} on ${background} = ${ratio.toFixed(2)}`);
    }
  }
  expect(problems).toEqual([]);
}

describe('Project Write – Phase 1 smoke test', () => {
  before(async () => {
    await pinMainWindow();
    await $('h1').waitForExist({ timeout: 60_000 });
  });

  it('starts and shows the start screen', async () => {
    await expect($('h1')).toHaveText('Project Write');
    await waitForChecks();
    const rows = await $$('[data-check]').getElements();
    expect(rows).toHaveLength(4);
    for (const row of rows) {
      expect(await row.getAttribute('data-passed')).toBe('true');
    }
  });

  it('enforces the CSP: no inline script, no image from the web', async () => {
    const result = await browser.execute(
      () =>
        new Promise<{ inlineRan: boolean; imageLoaded: boolean; violations: string[] }>((resolve) => {
          const violations: string[] = [];
          document.addEventListener('securitypolicyviolation', (event) => {
            violations.push(event.effectiveDirective || event.violatedDirective);
          });
          const flags = window as unknown as { __pwInlineRan?: boolean };
          flags.__pwInlineRan = false;

          const script = document.createElement('script');
          script.textContent = 'window.__pwInlineRan = true;';
          document.head.appendChild(script);

          const image = new Image();
          const finish = (imageLoaded: boolean) => {
            setTimeout(() => {
              resolve({ inlineRan: flags.__pwInlineRan === true, imageLoaded, violations });
            }, 250);
          };
          image.onload = () => {
            finish(true);
          };
          image.onerror = () => {
            finish(false);
          };
          image.src = 'https://example.com/project-write-csp-probe.png';
        }),
    );
    expect(result.inlineRan).toBe(false);
    expect(result.imageLoaded).toBe(false);
    expect(result.violations.some((d) => d.startsWith('script-src'))).toBe(true);
    expect(result.violations.some((d) => d.startsWith('img-src'))).toBe(true);
  });

  it('rejects file system, shell, sql and path access from the frontend', async () => {
    const outcome = await browser.execute(async () => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      const attempts: [string, unknown][] = [
        ['plugin:fs|read_text_file', { path: '/etc/hosts' }],
        ['plugin:fs|write_text_file', { path: 'pw-e2e.txt', data: 'x' }],
        ['plugin:fs|read_dir', { path: '.' }],
        ['plugin:shell|execute', { program: 'sh', args: ['-c', 'echo hi'] }],
        ['plugin:sql|load', { db: 'sqlite:pw-e2e.db' }],
        ['plugin:path|resolve_directory', { directory: 11 }],
        ['plugin:opener|open_path', { path: '/' }],
        // Folder dialogs are shown by Rust only (no dialog plugin, section 8).
        ['plugin:dialog|open', { directory: true }],
        // App commands take tokens, never paths.
        ['library_open', { request: { location: '/etc', acceptMoved: true } }],
      ];
      const results: Record<string, string> = {};
      for (const [command, args] of attempts) {
        try {
          await internals.invoke(command, args);
          results[command] = 'allowed';
        } catch {
          results[command] = 'rejected';
        }
      }
      const allowed = await internals.invoke('system_check', { request: { checks: ['database'] } });
      return {
        results,
        allowed,
        globalTauri: typeof (window as unknown as { __TAURI__?: unknown }).__TAURI__,
        frozen: Object.isFrozen(Object.prototype),
      };
    });

    for (const [command, result] of Object.entries(outcome.results)) {
      expect(`${command}: ${result}`).toBe(`${command}: rejected`);
    }
    expect(outcome.allowed).toMatchObject({ results: [{ check: 'database', passed: true }] });
    expect(outcome.globalTauri).toBe('undefined');
    expect(outcome.frozen).toBe(true);
  });

  it('meets the text contrast of section 6 in the theme it starts with', async () => {
    // The app follows the system setting at start (section 11).
    const startTheme = await browser.execute(() => document.documentElement.dataset['theme'] ?? '');
    expect(['dark', 'light']).toContain(startTheme);
    await expectContrast(startTheme as ThemeName);
  });

  it('puts a button first in the tab order and shows a visible focus ring', async () => {
    // The embedded WebDriver sends synthetic key events, and browsers ignore
    // untrusted Tab presses for focus navigation. So the tab order is checked
    // structurally and focus is set programmatically (see ADR 0003).
    const focused = await browser.execute(() => {
      const tabbable = Array.from(
        document.querySelectorAll<HTMLElement>('a[href], button, input, select, textarea, [tabindex]'),
      ).filter(
        (element) =>
          element.tabIndex >= 0 &&
          !(element as HTMLButtonElement).disabled &&
          element.getClientRects().length > 0,
      );
      const first = tabbable[0];
      if (!first) {
        return null;
      }
      first.focus({ focusVisible: true });
      const active = document.activeElement as HTMLElement;
      return {
        // Every button is reachable: none is taken out of the tab order.
        allButtonsTabbable: Array.from(document.querySelectorAll('button')).every(
          (button) => button.disabled || tabbable.includes(button),
        ),
        checkButtonTabbable: tabbable.some((element) => element.dataset['testid'] === 'check-retry'),
        tag: active.tagName,
        isFirst: active === first,
        focusVisible: active.matches(':focus-visible'),
      };
    });
    expect(focused).toMatchObject({
      allButtonsTabbable: true,
      checkButtonTabbable: true,
      tag: 'BUTTON',
      isFirst: true,
      focusVisible: true,
    });
    // Section 6: graphical elements need 3:1 against their surroundings.
    // Measured on the rendered pixels, like the text contrast.
    const { image, scale } = await settledFrame();
    const ring = await browser.execute(() => {
      const rect = (document.activeElement as HTMLElement).getBoundingClientRect();
      return {
        left: rect.left,
        middle: rect.top + rect.height / 2,
        theme: document.documentElement.dataset['theme'] ?? '',
      };
    });
    const expectedRing = themes[ring.theme as ThemeName].accent;
    // The ring lies 2px (outline-offset) left of the button. Scan the row of
    // pixels from 12px left of the button up to its edge: the ring must show
    // up there in the accent colour, at least 2px wide (section 11: visible
    // focus; index.css sets 2px, WebKit on macOS draws 3px), separated from
    // the card background.
    const y = Math.round(ring.middle * scale);
    const row: string[] = [];
    for (let x = Math.floor((ring.left - 12) * scale); x < Math.floor(ring.left * scale); x++) {
      row.push(pixelHex(image, x, y));
    }
    const ringPixels = row.filter((pixel) => colourDistance(pixel, expectedRing) <= PIXEL_TOLERANCE);
    const outsidePixel = row[0] ?? '#000000';
    const ratio = ringPixels[0] ? contrastRatio(ringPixels[0], outsidePixel) : 0;
    const problems: string[] = [];
    if (ringPixels.length / scale < 2 || ratio < 3) {
      problems.push(
        `rendered ring ${String(ringPixels.length)} px of ${expectedRing} next to ${outsidePixel} = ${ratio.toFixed(2)}; ` +
          `row (scale ${scale.toFixed(2)}): ${row.join(' ')}`,
      );
    }
    expect(problems).toEqual([]);

    await $('[data-testid="check-retry"]').click();
    await waitForChecks();
  });

  // Runs last: switches the theme at runtime exactly like applyTheme().
  for (const theme of ['dark', 'light'] as ThemeName[]) {
    it(`keeps the text contrast after switching to the ${theme} theme at runtime`, async () => {
      await browser.execute((scheme: string) => {
        document.documentElement.dataset['theme'] = scheme;
      }, theme);
      await expectContrast(theme);
    });
  }
});
