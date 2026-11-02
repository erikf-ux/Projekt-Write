/**
 * Phase 1 acceptance tests in the running app (section 25):
 * start, CSP, rejected file system access, contrast, keyboard focus.
 */
import { browser, $, $$, expect } from '@wdio/globals';
import { contrastRatio, MIN_TEXT_CONTRAST } from '../../../src/modules/themes/contrast';
import { themes, type ThemeName } from '../../../src/modules/themes/tokens';

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

/** Asserts that `theme` is active and every text reaches 4.5:1 (section 6). */
function expectContrast(samples: ContrastSample[], theme: ThemeName) {
  expect(samples.length).toBeGreaterThan(10);
  expect(samples[0]?.theme).toBe(theme);
  expect(rgbToHex(samples[0]?.mainBackground ?? '').toUpperCase()).toBe(themes[theme].surface.toUpperCase());
  // One compact line per failing element: selector, text, colours, ratio.
  const failures = samples
    .filter((sample) => sample.text !== '' && sample.text !== '·')
    .map((sample) => ({
      ...sample,
      ratio: contrastRatio(rgbToHex(sample.color), rgbToHex(sample.background)),
    }))
    .filter((sample) => sample.ratio < MIN_TEXT_CONTRAST)
    .map(
      (sample) =>
        `${sample.selector} "${sample.text}": ${sample.color} on ${sample.background} = ${sample.ratio.toFixed(2)} ` +
        `(--pw-color-text ${sample.textVariable})`,
    );
  expect(failures).toEqual([]);
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
    expectContrast(await measureStartScreen(), startTheme as ThemeName);
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
        outlineWidth: style.outlineWidth,
        outlineColor: style.outlineColor,
        background: getComputedStyle(document.body).backgroundColor,
      };
    });
    expect(focused).toMatchObject({
      tabbable: 1,
      tag: 'BUTTON',
      isFirst: true,
      focusVisible: true,
      outlineStyle: 'solid',
    });
    if (focused) {
      // index.css sets 2px; WebKit on macOS reports a wider ring (3px). Section 11
      // requires a visible focus, so at least 2px is accepted.
      expect(parseFloat(focused.outlineWidth)).toBeGreaterThanOrEqual(2);
      // Section 6: graphical elements need 3:1 against their surroundings.
      const ring = contrastRatio(rgbToHex(focused.outlineColor), rgbToHex(focused.background));
      expect(ring >= 3 ? 'ok' : `${focused.outlineColor} on ${focused.background} = ${ring.toFixed(2)}`).toBe(
        'ok',
      );
    }

    await $('button').click();
    await waitForChecks();
  });

  // Runs last: switches the theme at runtime exactly like applyTheme().
  for (const theme of ['dark', 'light'] as ThemeName[]) {
    it(`keeps the text contrast after switching to the ${theme} theme at runtime`, async () => {
      await browser.execute((scheme: string) => {
        document.documentElement.dataset['theme'] = scheme;
      }, theme);
      expectContrast(await measureStartScreen(), theme);
    });
  }
});
