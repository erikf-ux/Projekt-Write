/**
 * Phase 1 acceptance tests in the running app (section 25):
 * start, CSP, rejected file system access, contrast, keyboard focus.
 */
import { browser, $, $$, expect } from '@wdio/globals';
import { contrastRatio, MIN_TEXT_CONTRAST } from '../../../src/modules/themes/contrast';
import { themes, type ThemeName } from '../../../src/modules/themes/tokens';
import { decodePng, pixelHex, type Image } from '../png';

const READY_TEXTS = ['Alles bereit.', 'Everything is ready.'];

async function waitForChecks() {
  const summary = $('[data-testid="check-summary"]');
  await summary.waitUntil(async () => READY_TEXTS.includes(await summary.getText()), {
    timeoutMsg: 'system check did not report "ready"',
  });
}

function rgbToHex(rgb: string): string {
  const match = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(rgb);
  if (!match) {
    throw new Error(`Unexpected colour: ${rgb}`);
  }
  return `#${[match[1], match[2], match[3]].map((v) => Number(v).toString(16).padStart(2, '0')).join('')}`;
}

interface ContrastSample {
  selector: string;
  text: string;
  color: string;
  background: string;
  theme: string;
  surfaceVariable: string;
  textVariable: string;
  mainBackground: string;
}

/** Measures text colour and effective background of every text element. */
async function measureStartScreen(): Promise<ContrastSample[]> {
  return browser.execute(async () => {
    document.body.getBoundingClientRect(); // forces style and layout
    await new Promise<void>((resolve) => {
      const fallback = setTimeout(resolve, 250);
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          clearTimeout(fallback);
          resolve();
        });
      });
    });
    const background = (element: Element | null): string => {
      for (let node = element; node; node = node.parentElement) {
        const color = getComputedStyle(node).backgroundColor;
        if (color && color !== 'transparent' && !/rgba\(.*,\s*0\)$/.test(color)) {
          return color;
        }
      }
      return getComputedStyle(document.body).backgroundColor;
    };
    const main = document.querySelector('main');
    const mainStyle = main ? getComputedStyle(main) : null;
    const context = {
      theme: document.documentElement.dataset['theme'] ?? '',
      surfaceVariable: mainStyle?.getPropertyValue('--pw-color-surface').trim() ?? '',
      textVariable: mainStyle?.getPropertyValue('--pw-color-text').trim() ?? '',
      mainBackground: mainStyle?.backgroundColor ?? '',
    };
    const selectors = [
      'h1',
      'header p',
      'h2',
      '[data-testid="check-summary"]',
      '[data-check] span',
      'button',
      'footer span',
    ];
    return selectors.flatMap((selector) =>
      Array.from(document.querySelectorAll(selector)).map((element) => ({
        selector,
        text: element.textContent.trim(),
        color: getComputedStyle(element).color,
        background: background(element),
        ...context,
      })),
    );
  });
}

/** Every problem with `theme` on the start screen, one compact line each (section 6). */
function contrastFailures(samples: ContrastSample[], theme: ThemeName): string[] {
  const problems: string[] = [];
  if (samples.length <= 10) {
    problems.push(`only ${String(samples.length)} text elements measured`);
  }
  if (samples[0]?.theme !== theme) {
    problems.push(`data-theme is "${samples[0]?.theme ?? ''}", expected "${theme}"`);
  }
  const surface = rgbToHex(samples[0]?.mainBackground ?? 'rgb(0, 0, 0)').toUpperCase();
  if (surface !== themes[theme].surface.toUpperCase()) {
    problems.push(`main background ${surface}, expected ${themes[theme].surface}`);
  }
  for (const sample of samples) {
    if (sample.text === '' || sample.text === '·') {
      continue;
    }
    const ratio = contrastRatio(rgbToHex(sample.color), rgbToHex(sample.background));
    if (ratio < MIN_TEXT_CONTRAST) {
      problems.push(
        `${sample.selector} "${sample.text}": ${sample.color} on ${sample.background} = ${ratio.toFixed(2)} ` +
          `(--pw-color-text ${sample.textVariable})`,
      );
    }
  }
  return problems;
}

/**
 * Renders a frame and returns it as an image.
 *
 * The CI windows on Windows and macOS do not render on their own: until the
 * next frame, getComputedStyle there still reports colours from before a
 * change (measured in CI: values one theme switch behind, all correct after
 * a forced frame). A WebDriver screenshot renders a frame, so every check
 * measures what is really on screen (docs/adr/0003).
 */
async function renderFrame(): Promise<{ image: Image; scale: number }> {
  const image = decodePng(Buffer.from(await browser.takeScreenshot(), 'base64'));
  const viewportWidth = await browser.execute(() => window.innerWidth);
  return { image, scale: image.width / viewportWidth };
}

/** Distance between two `#RRGGBB` colours (largest channel difference). */
function colourDistance(a: string, b: string): number {
  const channels = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  const [x, y] = [channels(a), channels(b)];
  return Math.max(...x.map((value, i) => Math.abs(value - (y[i] ?? 0))));
}

/** Rendering may convert colour spaces slightly (macOS); allow small deviations. */
const PIXEL_TOLERANCE = 8;

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

/**
 * Asserts that `theme` is rendered and every text reaches 4.5:1 (section 6):
 * computed colours after a rendered frame, plus the rendered background pixels.
 */
async function expectContrast(theme: ThemeName) {
  const { image, scale } = await renderFrame();
  const problems = contrastFailures(await measureStartScreen(), theme);
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
  expect(problems).toEqual([]);
}

describe('Project Write – Phase 1 smoke test', () => {
  before(async () => {
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

  it('puts the button first in the tab order and shows a visible focus ring', async () => {
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
      const style = getComputedStyle(active);
      return {
        tabbable: tabbable.length,
        tag: active.tagName,
        isFirst: active === first,
        focusVisible: active.matches(':focus-visible'),
        outlineStyle: style.outlineStyle,
      };
    });
    expect(focused).toMatchObject({
      tabbable: 1,
      tag: 'BUTTON',
      isFirst: true,
      focusVisible: true,
      outlineStyle: 'solid',
    });
    // Section 6: graphical elements need 3:1 against their surroundings.
    // Measured on a rendered frame: computed style and the ring's pixels.
    const { image, scale } = await renderFrame();
    const ring = await browser.execute(() => {
      const button = document.activeElement as HTMLElement;
      const style = getComputedStyle(button);
      const rect = button.getBoundingClientRect();
      return {
        width: style.outlineWidth,
        color: style.outlineColor,
        left: rect.left,
        middle: rect.top + rect.height / 2,
      };
    });
    const problems: string[] = [];
    // index.css sets 2px; WebKit on macOS reports 3px. Section 11 asks for a
    // visible focus, so at least 2px is accepted.
    if (parseFloat(ring.width) < 2) {
      problems.push(`outline width ${ring.width}`);
    }
    const currentTheme = await browser.execute(() => document.documentElement.dataset['theme'] ?? '');
    const expectedRing = themes[currentTheme as ThemeName].accent;
    if (colourDistance(rgbToHex(ring.color), expectedRing) > PIXEL_TOLERANCE) {
      problems.push(`computed outline colour ${ring.color}, expected ${expectedRing}`);
    }
    // The ring starts 2px (outline-offset) left of the button; sample its middle
    // and the card background just outside it.
    const y = Math.round(ring.middle * scale);
    const ringPixel = pixelHex(image, Math.round((ring.left - 3) * scale), y);
    const outsidePixel = pixelHex(image, Math.round((ring.left - 8) * scale), y);
    const ratio = contrastRatio(ringPixel, outsidePixel);
    if (colourDistance(ringPixel, expectedRing) > PIXEL_TOLERANCE || ratio < 3) {
      problems.push(`rendered ring ${ringPixel} next to ${outsidePixel} = ${ratio.toFixed(2)}`);
    }
    expect(problems).toEqual([]);

    await $('button').click();
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
