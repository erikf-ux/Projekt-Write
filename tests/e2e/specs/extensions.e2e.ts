/**
 * Phase 13, no foreign code (ADR 0027, decision 3), in the real app with
 * the CSP of `tauri.conf.json` (`script-src 'self'`): text cannot be run
 * as code, and no script or module comes from an inline element, a
 * `data:` or `blob:` address or another host.
 *
 * How the checks run: the embedded WebDriver evaluates the test functions
 * natively in the page (`evaluate_javascript` and the like), which is not
 * subject to the CSP; that is the test's own door, not one the app offers.
 * Everything the functions then try (`eval`, `new Function`, adding a
 * `<script>`, `import()`) goes through the page's CSP like code of the app
 * would. Each attempt would set a global flag; the flag must stay unset,
 * and for scripts and modules the CSP must report a violation, so the
 * attempt failed because of the policy and not by accident. A same-origin
 * `import()` must still work, so the failing imports prove something.
 *
 * Registering after start: the registries are module state of the bundle
 * and never put on `window` (`src/extensions/registries.ts`). The frontend
 * of the e2e build is the release bundle (`npm run build:e2e` runs the
 * normal `vite build`; only Rust has the `e2e` feature), so a test-only
 * door to the registries would ship in the release too. This file
 * therefore checks that nothing on `window` offers `register`; that
 * `register` throws after `seal()` in the started app is checked in Vitest
 * (`src/extensions/builtin.test.ts`, with the start sequence of `main.tsx`
 * in `src/test/setup.ts`).
 *
 * Runs after language.e2e.ts in the same library.
 */
import { $, browser, expect } from '@wdio/globals';
import { closeSettings, openSettings, pinMainWindow } from '../session';

type Attempt =
  | 'eval'
  | 'function'
  | 'timeout'
  | 'inline'
  | 'data'
  | 'blob'
  | 'remote'
  | 'import-data'
  | 'import-blob'
  | 'import-remote'
  | 'import-self';

interface Outcome {
  /** The foreign code ran (set the flag). */
  ran: boolean;
  /** What the attempt threw or rejected with (`Name: message`), or null. */
  error: string | null;
  /** `load`, `error` or `timeout` for script elements; null otherwise. */
  script: string | null;
  /** CSP violations during the attempt: `directive blocked-uri`. */
  violations: string[];
  /** The policy the violations name. */
  policy: string | null;
}

/** An address on another host (the app has no network access either way). */
const REMOTE = 'https://example.org/project-write-foreign.js';

/** Runs one attempt in the page and reports what happened. */
async function attempt(kind: Attempt): Promise<Outcome> {
  return browser.execute(
    async (which: Attempt, remote: string): Promise<Outcome> => {
      const flag = '__pwForeignCodeRan';
      const global = window as unknown as Record<string, unknown>;
      global[flag] = false;
      const code = `window.${flag} = true;`;
      const violations: string[] = [];
      let policy: string | null = null;
      const onViolation = (event: SecurityPolicyViolationEvent) => {
        violations.push(`${event.effectiveDirective} ${event.blockedURI}`);
        policy = event.originalPolicy;
      };
      document.addEventListener('securitypolicyviolation', onViolation);
      const pause = (ms: number) =>
        new Promise<void>((resolve) => {
          setTimeout(resolve, ms);
        });
      /** Adds a script element and waits for its load or error event. */
      const loadScript = (source: string) =>
        new Promise<string>((resolve) => {
          const script = document.createElement('script');
          const done = (result: string) => {
            script.remove();
            resolve(result);
          };
          script.addEventListener('load', () => {
            done('load');
          });
          script.addEventListener('error', () => {
            done('error');
          });
          setTimeout(() => {
            done('timeout');
          }, 3_000);
          script.src = source;
          document.head.append(script);
        });
      const dataUrl = `data:text/javascript,${encodeURIComponent(code)}`;
      const blobUrl = () => URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
      let error: string | null = null;
      let script: string | null = null;
      try {
        switch (which) {
          case 'eval':
            window.eval(code);
            break;
          case 'function':
            // eslint-disable-next-line @typescript-eslint/no-implied-eval -- what the page must not be able to do
            (new Function(code) as () => void)();
            break;
          case 'timeout':
            // eslint-disable-next-line @typescript-eslint/no-implied-eval -- what the page must not be able to do
            setTimeout(code, 0);
            break;
          case 'inline': {
            const inline = document.createElement('script');
            inline.textContent = code;
            document.head.append(inline);
            inline.remove();
            break;
          }
          case 'data':
            script = await loadScript(dataUrl);
            break;
          case 'blob': {
            const url = blobUrl();
            script = await loadScript(url);
            URL.revokeObjectURL(url);
            break;
          }
          case 'remote':
            script = await loadScript(remote);
            break;
          case 'import-data':
            await import(dataUrl);
            break;
          case 'import-blob': {
            const url = blobUrl();
            try {
              await import(url);
            } finally {
              URL.revokeObjectURL(url);
            }
            break;
          }
          case 'import-remote':
            await import(remote);
            break;
          case 'import-self': {
            // The app's own entry module: already loaded, so importing it
            // again runs nothing and resolves.
            const own = document.querySelector<HTMLScriptElement>('script[type="module"][src]');
            if (!own) {
              throw new Error('no module script in the page');
            }
            await import(own.src);
            break;
          }
        }
      } catch (caught) {
        error = caught instanceof Error ? `${caught.name}: ${caught.message}` : String(caught);
      }
      // Violation events and timers are queued tasks.
      await pause(500);
      document.removeEventListener('securitypolicyviolation', onViolation);
      const ran = global[flag] === true;
      global[flag] = false;
      return { ran, error, script, violations, policy };
    },
    kind,
    REMOTE,
  );
}

/** The sources of `script-src` in a policy. */
function scriptSources(policy: string): string[] {
  const directive = policy
    .split(';')
    .map((part) => part.trim().split(/\s+/))
    .find(([name]) => name === 'script-src');
  return directive?.slice(1) ?? [];
}

/** A script-src violation was reported (Chromium names `script-src-elem`). */
function blockedByScriptSrc(outcome: Outcome): boolean {
  return outcome.violations.some((violation) => violation.startsWith('script-src'));
}

describe('No foreign code (Phase 13)', () => {
  before(async () => {
    await pinMainWindow();
    await closeSettings();
  });

  it('cannot run text as code: eval, new Function and setTimeout with a string', async () => {
    const evaluated = await attempt('eval');
    expect(evaluated.ran).toBe(false);
    expect(evaluated.error).toMatch(/^EvalError/);

    const constructed = await attempt('function');
    expect(constructed.ran).toBe(false);
    expect(constructed.error).toMatch(/^EvalError/);

    const timer = await attempt('timeout');
    expect(timer.ran).toBe(false);
  });

  it('does not run an inline script element', async () => {
    const outcome = await attempt('inline');
    expect(outcome.ran).toBe(false);
    expect(blockedByScriptSrc(outcome)).toBe(true);
  });

  it('does not load scripts from data:, blob: or another host', async () => {
    for (const kind of ['data', 'blob', 'remote'] as const) {
      const outcome = await attempt(kind);
      expect({ kind, ran: outcome.ran }).toEqual({ kind, ran: false });
      expect({ kind, script: outcome.script }).not.toEqual({ kind, script: 'load' });
      expect({ kind, blocked: blockedByScriptSrc(outcome) }).toEqual({ kind, blocked: true });
    }
  });

  it('rejects import() of data:, blob: and another host, but imports its own module', async () => {
    const own = await attempt('import-self');
    expect(own.error).toBeNull();
    for (const kind of ['import-data', 'import-blob', 'import-remote'] as const) {
      const outcome = await attempt(kind);
      expect({ kind, ran: outcome.ran }).toEqual({ kind, ran: false });
      expect({ kind, rejected: outcome.error !== null }).toEqual({ kind, rejected: true });
    }
  });

  it('runs under a policy that allows scripts from the app only', async () => {
    const outcome = await attempt('inline');
    expect(outcome.policy).not.toBeNull();
    const sources = scriptSources(outcome.policy ?? '');
    expect(sources).toContain("'self'");
    for (const forbidden of ["'unsafe-eval'", "'unsafe-inline'", 'data:', 'blob:', '*', 'https:', 'http:']) {
      expect(sources).not.toContain(forbidden);
    }
    // A nonce on a script of the page could be copied onto a new inline
    // script; Tauri gives nonces only to scripts from http(s) addresses,
    // and the app has none.
    const nonces = await browser.execute(
      () => [...document.querySelectorAll('script')].filter((script) => script.nonce !== '').length,
    );
    expect(nonces).toBe(0);
  });

  it('offers no registry on window to register into after start', async () => {
    const exposed = await browser.execute(() =>
      Object.getOwnPropertyNames(window).filter((name) => {
        try {
          const value: unknown = (window as unknown as Record<string, unknown>)[name];
          return (
            typeof value === 'object' &&
            value !== null &&
            typeof (value as { register?: unknown }).register === 'function' &&
            typeof (value as { seal?: unknown }).seal === 'function'
          );
        } catch {
          return false;
        }
      }),
    );
    expect(exposed).toEqual([]);
  });

  it('lists the built-in extensions of every registry, read only, in Settings › Erweitert', async () => {
    await openSettings('advanced');
    const list = $('[data-testid="advanced-extensions"]');
    await list.waitForDisplayed();
    // Rust (formats, engines, exporters) and the interface (editor, Mesh).
    for (const id of ['pwrite', 'spelling', 'languagetool', 'pack', 'print', 'react-flow']) {
      await list.$(`[data-extension="${id}"]`).waitForExist({ timeoutMsg: `${id} is not listed` });
    }
    // Nothing to install, switch on or remove: no controls in the list.
    expect(await list.$$('button, input, select').length).toBe(0);
    await closeSettings();
  });
});
