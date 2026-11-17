/**
 * Phase 12, language check (section 14, ADR 0023 to 0025), in the real
 * app with the shipped dictionaries: underlines, suggestions in the
 * context menu, own dictionaries, code left out, the simple rules, the
 * autocorrect with its own undo step and rejection, and switching off.
 *
 * Runs after mesh.e2e.ts in the same library.
 */
import { $, browser, expect } from '@wdio/globals';
import { activeEditor, newDocument, runFromPalette, typeText } from '../editing';
import { closeSettings, openSettings, pinMainWindow } from '../session';

async function invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
  return browser.execute(
    async (name: string, body: Record<string, unknown>) => {
      const internals = (
        window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }
      ).__TAURI_INTERNALS__;
      return internals.invoke(name, { request: body });
    },
    command,
    request,
  ) as Promise<T>;
}

const SURFACE = '[role="tabpanel"]:not([hidden]) [data-testid="editor"]';

/** The text of the active document, without the space typed at its end. */
async function text(): Promise<string> {
  return (await (await activeEditor()).getText()).trim();
}

/** The underlined texts of the active document, by kind. */
async function underlined(kind: 'spelling' | 'grammar' | 'punctuation'): Promise<string[]> {
  return browser.execute(
    (surface: string, wanted: string) =>
      [...document.querySelectorAll(`${surface} .pw-check-${wanted}`)].map((element) => element.textContent),
    SURFACE,
    kind,
  );
}

/** Opens the context menu on the first underline with this text (right click). */
async function openMenuOn(word: string): Promise<void> {
  await browser.execute(
    (surface: string, wanted: string) => {
      const target = [...document.querySelectorAll<HTMLElement>(`${surface} .pw-check`)].find(
        (element) => element.textContent === wanted,
      );
      if (!target) {
        throw new Error(`no underline on ${wanted}`);
      }
      const box = target.getBoundingClientRect();
      target.dispatchEvent(
        new MouseEvent('contextmenu', {
          bubbles: true,
          cancelable: true,
          clientX: box.left + box.width / 2,
          clientY: box.top + box.height / 2,
        }),
      );
    },
    SURFACE,
    word,
  );
  await $('[data-testid="context-menu"]').waitForDisplayed({ timeoutMsg: 'no context menu' });
}

async function waitForUnderline(kind: 'spelling' | 'grammar' | 'punctuation', word: string): Promise<void> {
  await browser.waitUntil(async () => (await underlined(kind)).includes(word), {
    timeout: 15_000,
    timeoutMsg: `${word} is not underlined as ${kind}`,
  });
}

describe('Language check (Phase 12)', () => {
  before(async () => {
    await pinMainWindow();
    await closeSettings();
  });

  it('underlines a misspelt word and replaces it with a suggestion from the context menu', async () => {
    await newDocument();
    await (await activeEditor()).click();
    await typeText('Das Fesnter steht offen.');
    await waitForUnderline('spelling', 'Fesnter');
    expect(await $('[data-testid="status-language"]').getText()).toContain('de-DE');
    await openMenuOn('Fesnter');
    // At most five suggestions; the engine's order may vary, so by name.
    await $('[data-testid="context-menu"] [data-item="suggestion-0"]').waitForDisplayed({
      timeoutMsg: 'no suggestion',
    });
    const labels = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="context-menu"] [data-item^="suggestion-"]')].map(
        (item) => item.textContent,
      ),
    );
    expect(labels.length).toBeLessThanOrEqual(5);
    expect(labels).toContain('Fenster');
    await $(
      `[data-testid="context-menu"] [data-item="suggestion-${String(labels.indexOf('Fenster'))}"]`,
    ).click();
    await browser.waitUntil(async () => (await text()).includes('Das Fenster steht offen.'));
    await browser.waitUntil(async () => (await underlined('spelling')).length === 0, {
      timeoutMsg: 'the corrected word stays underlined',
    });
  });

  it('adds an unknown compound to the own dictionary with one click', async () => {
    await typeText(' Am Flohmarktstand.');
    await waitForUnderline('spelling', 'Flohmarktstand');
    await openMenuOn('Flohmarktstand');
    await $('[data-testid="context-menu"] [data-item="add"]').click();
    await browser.waitUntil(async () => !(await underlined('spelling')).includes('Flohmarktstand'), {
      timeoutMsg: 'the added word stays underlined',
    });
    const dictionaries = await invoke<{ name: string; forAll: boolean; words: number }[]>(
      'dictionaries_list',
      {},
    );
    expect(dictionaries.some((d) => d.forAll && d.words >= 1)).toBe(true);
  });

  it('leaves inline code out and marks double spaces and repeated words blue', async () => {
    await newDocument();
    await (await activeEditor()).click();
    // Typed with backticks: the input rule makes inline code of it.
    await typeText('Nimm `Fesnter` hier. Das ist ist  gut.');
    await waitForUnderline('punctuation', '  ');
    await waitForUnderline('grammar', ' ist');
    expect(await underlined('spelling')).toEqual([]);
    // Shift+F10 on the repetition offers to remove it.
    await openMenuOn(' ist');
    await $('[data-testid="context-menu"] [data-item="replacement-0"]').click();
    await browser.waitUntil(async () => (await text()).includes('Das ist  gut.'));
  });

  it('corrects a unique error automatically as one undo step and remembers a rejection', async () => {
    await openSettings('language');
    const autocorrect = $('[data-testid="language-autocorrect"]');
    await autocorrect.waitForDisplayed();
    if (!(await autocorrect.isSelected())) {
      await autocorrect.click();
    }
    await closeSettings();
    await newDocument();
    await (await activeEditor()).click();
    // One word one step away: Zimmer. (Fesnter has two: Fenster, Fester.)
    await typeText('Das Zimemr ');
    await browser.waitUntil(async () => (await text()) === 'Das Zimmer', {
      timeout: 10_000,
      timeoutMsg: 'not corrected',
    });
    // Ctrl+Z brings the typed word back and records the rejection.
    await browser.keys(['Control', 'z']);
    await browser.waitUntil(async () => (await text()) === 'Das Zimemr', {
      timeoutMsg: 'Ctrl+Z did not restore the typed word',
    });
    await browser.waitUntil(
      async () =>
        (await invoke<{ word: string; correction: string }[]>('autocorrect_rejections', {})).some(
          (r) => r.word === 'Zimemr' && r.correction === 'Zimmer',
        ),
      { timeoutMsg: 'the rejection was not recorded' },
    );
    // The rejected correction is not made again; ambiguous errors stay.
    await typeText('und Zimemr und Fesnter und Mitter ');
    await browser.pause(1500);
    expect(await text()).toBe('Das Zimemr und Zimemr und Fesnter und Mitter');
  });

  it('turns the language check off and on', async () => {
    await waitForUnderline('spelling', 'Mitter');
    await runFromPalette('language.toggle');
    await browser.waitUntil(async () => (await underlined('spelling')).length === 0, {
      timeoutMsg: 'underlines stay while the check is off',
    });
    expect(await $('[data-testid="status-language"]').getAttribute('data-off')).toBe('true');
    await runFromPalette('language.toggle');
    await waitForUnderline('spelling', 'Mitter');
    // Leave the autocorrect off for the files after this one.
    await openSettings('language');
    const autocorrect = $('[data-testid="language-autocorrect"]');
    if (await autocorrect.isSelected()) {
      await autocorrect.click();
    }
    await closeSettings();
  });
});
