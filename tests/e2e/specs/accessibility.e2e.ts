/**
 * Phase 14, "Prüfung der Barrierefreiheit" (section 6: "Alle Funktionen per
 * Tastatur, Kontrast mindestens 4,5:1 für Text und 3:1 für grafische
 * Elemente"; section 11: visible focus, ARIA labels).
 *
 * What earlier files already prove, and this one does not repeat:
 *
 * * smoke.e2e.ts: start screen – every button in the tab order, visible
 *   focus ring with 3:1, text contrast in both themes (rendered pixels);
 * * workbench.e2e.ts: every button of the workbench reachable (composite
 *   widgets with one tab stop), a visible focus in every region, Explorer
 *   and tabs by keyboard, Shift+F10 in the Explorer, the palette runs a
 *   command, shortcut conflicts;
 * * structure.e2e.ts: subnets from the context menu (Shift+F10);
 * * search.e2e.ts: Ctrl+Shift+F and the hits by keyboard;
 * * mesh.e2e.ts: Mesh views, connecting, deleting and undoing by keyboard,
 *   Shift+F10 on the selection;
 * * language.e2e.ts: suggestions from the context menu of the writing
 *   surface (Shift+F10);
 * * tokens.test.ts (Vitest): every token pair of both themes.
 *
 * This file adds: every registered command is in the command palette (or
 * deliberately hidden there, with the reason), the palette and the dialogs
 * keep the focus inside, close with Escape and give the focus back, the
 * sections of the settings by arrow keys, the regions by their shortcuts,
 * the context menu of the tabs, the ARIA rules of a11y.ts in the
 * workbench, palette, dialogs, gallery and Mesh, and text and icon
 * contrast of the workbench, palette and settings in both themes.
 *
 * The embedded WebDriver sends synthetic key events (ADR 0003): Tab does
 * not move the focus by itself, so the focus trap is checked where the app
 * handles Tab (at the first and last control), and focus is set in the
 * page where only the browser would move it.
 *
 * Runs after settings-transfer.e2e.ts, in the library of the earlier files.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import { MIN_GRAPHIC_CONTRAST, MIN_TEXT_CONTRAST } from '../../../src/modules/themes/contrast';
import type { ThemeName } from '../../../src/modules/themes/tokens';
import { ariaProblems, renderedContrast, screenItems } from '../a11y';
import { activeEditor, typeText } from '../editing';
import { measured } from '../measurements';
import { settledFrame } from '../pixels';
import { root } from '../sandbox';
import { closeSettings, pinMainWindow } from '../session';

/** Every command id of the registry (the `CommandId` union in types.ts). */
function registeredCommands(): string[] {
  const source = fs.readFileSync(path.join(root, 'src', 'modules', 'commands', 'types.ts'), 'utf8');
  const union = /export type CommandId =([\s\S]*?);/.exec(source)?.[1] ?? '';
  return [...union.matchAll(/'([^']+)'/g)].map((match) => match[1] ?? '');
}

/**
 * Commands the palette may leave out in the state of this file (a Markdown
 * document of the library open in rich text, saved, nothing selected, zoom
 * 100 %), and why: they do nothing here (section 11: "zeigt nur Befehle, die
 * in der aktuellen Phase funktionieren") and appear where they work.
 */
const HIDDEN_HERE: Record<string, string> = {
  'palette.show': 'the palette does not list itself',
  'document.save': 'everything is saved',
  'document.saveAs': 'only for an external file',
  'document.takeIntoLibrary': 'only for an external file',
  'document.unarchive': 'only for an archived document',
  'document.convertToMarkdown': 'the document already is Markdown',
  'document.assignTo': 'only when subnets exist',
  'trash.empty': 'only when the trash holds something',
  'structure.undo': 'only after a change of the structure',
  'structure.redo': 'only after an undo of the structure',
  'mesh.layout': 'only in the Mesh tab',
  'mesh.layoutAll': 'only in the Mesh tab',
  'find.next': 'only with the find bar open',
  'find.previous': 'only with the find bar open',
  'tab.closeOthers': 'only with more than one tab',
  'tab.next': 'only with more than one tab',
  'tab.previous': 'only with more than one tab',
  'tab.moveLeft': 'only when the tab is not the first',
  'tab.moveRight': 'only when the tab is not the last',
  'zoom.reset': 'the zoom is 100 %',
  'theme.system': 'the theme already chosen',
  'theme.light': 'the theme already chosen',
  'theme.dark': 'the theme already chosen',
  'format.imageEdit': 'only with an image selected',
  'language.part': 'only with text selected',
  'backup.create': 'not while a backup runs',
  'backup.check': 'not while a backup runs',
  'backup.restore': 'not while a backup runs',
};

/** Commands that open a dialog, run by keyboard from the palette. */
const DIALOGS: { command: string; dialog: string }[] = [
  { command: 'settings.open', dialog: 'settings-dialog' },
  { command: 'statistics.show', dialog: 'statistics-dialog' },
  { command: 'document.tags', dialog: 'tags-dialog' },
  { command: 'tags.manage', dialog: 'tag-manager' },
  { command: 'subnet.new', dialog: 'subnet-dialog' },
  { command: 'document.moveTo', dialog: 'subnet-picker' },
  { command: 'document.history', dialog: 'history-dialog' },
  { command: 'document.saveVersion', dialog: 'version-name-dialog' },
  { command: 'format.link', dialog: 'link-dialog' },
  { command: 'format.image', dialog: 'media-picker' },
  { command: 'language.document', dialog: 'language-profile' },
];

const SETTINGS_SECTIONS = ['general', 'language', 'shortcuts', 'library', 'advanced'];

/** What has the focus, as a short description. */
async function focused(): Promise<string> {
  return browser.execute(() => {
    const active = document.activeElement;
    if (!active || active === document.body) {
      return 'body';
    }
    const testId =
      active.getAttribute('data-testid') ?? active.closest('[data-testid]')?.getAttribute('data-testid');
    return `${active.tagName.toLowerCase()}${active.id ? `#${active.id}` : ''}[${testId ?? ''}]`;
  });
}

/** Puts the focus into the writing surface of the active tab. */
async function focusEditor(): Promise<string> {
  await browser.execute(() => {
    document.querySelector<HTMLElement>('[role="tabpanel"]:not([hidden]) [data-testid="editor"]')?.focus();
  });
  await browser.waitUntil(() =>
    browser.execute(() => document.activeElement?.getAttribute('data-testid') === 'editor'),
  );
  return focused();
}

async function paletteIds(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[data-testid="command-palette"] [role="option"]')].map(
      (option) => option.getAttribute('data-command') ?? '',
    ),
  );
}

/**
 * Runs a command with the keyboard only: Ctrl+Shift+P, its title typed,
 * arrow keys to the entry, Enter.
 */
async function runByKeyboard(command: string): Promise<void> {
  await browser.keys(['Control', 'Shift', 'p']);
  const palette = $('[data-testid="command-palette"]');
  await palette.waitForDisplayed();
  const option = $(`[data-testid="command-palette"] [data-command="${command}"]`);
  await option.waitForExist({ timeoutMsg: `${command} is not offered` });
  // The start of the title without the category ("Dokument: Statistik"):
  // it narrows the list, the arrow keys do the rest. Long typed texts and
  // characters such as "…" arrive unreliably under load (CI run 124).
  const fullTitle = await browser.execute(
    (id: string) =>
      document.querySelector(`[data-testid="command-palette"] [data-command="${id}"] > span`)?.lastChild
        ?.textContent ?? '',
    command,
  );
  const title = fullTitle.slice(0, 8).trimEnd();
  // The query must read exactly the title before the arrow keys look for
  // it. Under the network recorder on the CI runner the keys arrive slowly
  // (CI run 124: "Sprac" after 2 s), and the embedded WebDriver now and then
  // loses one (F-27): wait until the query stops changing, retype only when
  // it then reads something else.
  const query = () =>
    browser.execute(
      () =>
        document.querySelector<HTMLInputElement>('[data-testid="command-palette"] [role="combobox"]')
          ?.value ?? '',
    );
  const settled = async (): Promise<string> => {
    const deadline = Date.now() + 15_000;
    let last = await query();
    let unchanged = 0;
    while (Date.now() < deadline) {
      await browser.pause(250);
      const now = await query();
      if (now === title) {
        return now;
      }
      unchanged = now === last ? unchanged + 1 : 0;
      last = now;
      if (unchanged >= 8) {
        return now;
      }
    }
    return query();
  };
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const typed = await query();
    if (typed === title) {
      break;
    }
    if (typed !== '') {
      await browser.keys(['Control', 'a']);
      await browser.keys('Backspace');
    }
    await browser.keys(title.split(''));
    if ((await settled()) === title) {
      break;
    }
  }
  if ((await query()) !== title) {
    throw new Error(`could not type the title of ${command}: ${await query()}`);
  }
  for (let step = 0; step < 100; step += 1) {
    const active = await browser.execute(() => {
      const input = document.querySelector('[data-testid="command-palette"] [role="combobox"]');
      const id = input?.getAttribute('aria-activedescendant') ?? '';
      return document.getElementById(id)?.getAttribute('data-command') ?? '';
    });
    if (active === command) {
      // A lost Enter (F-27) leaves the palette open with the entry chosen.
      for (let press = 0; press < 3; press += 1) {
        await browser.keys('Enter');
        const closed = await palette
          .waitForExist({ reverse: true, timeout: 5_000 })
          .then(() => true)
          .catch(() => false);
        if (closed) {
          return;
        }
      }
      const state = await browser.execute(() => {
        const input = document.querySelector<HTMLInputElement>(
          '[data-testid="command-palette"] [role="combobox"]',
        );
        const active = document.activeElement;
        return {
          query: input?.value ?? null,
          chosen:
            document.getElementById(input?.getAttribute('aria-activedescendant') ?? '')?.textContent ?? null,
          focus: `${active?.tagName ?? ''} ${active?.getAttribute('data-testid') ?? ''} ${active?.getAttribute('role') ?? ''}`,
          dialogs: [...document.querySelectorAll('[role="dialog"]')].map((d) =>
            d.getAttribute('data-testid'),
          ),
        };
      });
      throw new Error(`the palette stayed open after Enter on ${command}: ${JSON.stringify(state)}`);
    }
    await browser.keys('ArrowDown');
  }
  throw new Error(`${command} could not be reached with the arrow keys`);
}

/** Checks the focus trap of the open dialog at its first and last control. */
async function trapProblems(selector: string): Promise<string[]> {
  const problems: string[] = [];
  const ends = await browser.execute((target: string) => {
    const dialog = document.querySelector(target);
    const focusable = [
      ...(dialog?.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ) ?? []),
    ].filter((element) => element.offsetParent !== null);
    focusable.forEach((element, index) => {
      element.dataset['pwTrap'] = String(index);
    });
    return focusable.length;
  }, selector);
  if (ends < 1) {
    return [`${selector}: nothing to focus`];
  }
  const trapIndex = () =>
    browser.execute(
      (target: string) =>
        document.querySelector(target)?.contains(document.activeElement)
          ? ((document.activeElement as HTMLElement).dataset['pwTrap'] ?? 'inside')
          : 'outside',
      selector,
    );
  const focus = (index: number) =>
    browser.execute(
      (target: string, at: number) => {
        document.querySelector<HTMLElement>(`${target} [data-pw-trap="${String(at)}"]`)?.focus();
      },
      selector,
      index,
    );
  await focus(ends - 1);
  await browser.keys('Tab');
  if ((await trapIndex()) !== '0') {
    problems.push(`${selector}: Tab on the last control went to ${await trapIndex()}`);
  }
  await focus(0);
  await browser.keys(['Shift', 'Tab']);
  if ((await trapIndex()) !== String(ends - 1)) {
    problems.push(`${selector}: Shift+Tab on the first control went to ${await trapIndex()}`);
  }
  // Focus that leaves the dialog (a click elsewhere, a late focus call) comes back.
  await browser.execute(() => {
    document.getElementById('pw-explorer-list')?.focus();
  });
  await browser
    .waitUntil(async () => (await trapIndex()) !== 'outside', { timeout: 2_000 })
    .catch(() => {
      problems.push(`${selector}: the focus stayed outside the dialog`);
    });
  await browser.execute(() => {
    for (const element of document.querySelectorAll<HTMLElement>('[data-pw-trap]')) {
      delete element.dataset['pwTrap'];
    }
  });
  return problems;
}

async function setTheme(theme: string): Promise<void> {
  await browser.execute((scheme: string) => {
    document.documentElement.dataset['theme'] = scheme;
  }, theme);
}

/** Text and graphic contrast of everything on screen inside `scope`. */
/** The lowest ratios per view, logged once per theme. */
const contrastSummary: string[] = [];

async function contrastProblems(
  theme: ThemeName,
  scope: string,
  label: string,
  minimumTexts = 5,
): Promise<string[]> {
  // The frame first: the boxes must describe what it shows (a list that
  // fills in late would move them).
  const { image, scale } = await settledFrame(theme);
  const items = await screenItems(scope);
  const problems: string[] = [];
  let texts = 0;
  let graphics = 0;
  let lowest = { text: Infinity, graphic: Infinity };
  for (const item of items) {
    const { foreground, background, ratio } = renderedContrast(image, scale, item);
    const minimum = item.kind === 'text' ? MIN_TEXT_CONTRAST : MIN_GRAPHIC_CONTRAST;
    if (item.kind === 'text') {
      texts += 1;
      lowest = { ...lowest, text: Math.min(lowest.text, ratio) };
    } else {
      graphics += 1;
      lowest = { ...lowest, graphic: Math.min(lowest.graphic, ratio) };
    }
    if (ratio < minimum) {
      problems.push(
        `${theme}, ${label}: ${item.kind} "${item.label}" ${foreground} on ${background} = ${ratio.toFixed(2)} (needs ${String(minimum)})`,
      );
    }
  }
  const low = (value: number) => (Number.isFinite(value) ? value.toFixed(2) : '–');
  contrastSummary.push(
    `${label} ${String(texts)} texts ≥ ${low(lowest.text)}, ${String(graphics)} icons and controls ≥ ${low(lowest.graphic)}`,
  );
  if (texts < minimumTexts) {
    problems.push(`${theme}, ${label}: only ${String(texts)} texts found`);
  }
  return problems;
}

describe('Accessibility (Phase 14)', () => {
  let startTheme = 'light';

  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    await closeSettings();
    startTheme = await browser.execute(() => document.documentElement.dataset['theme'] ?? 'light');
    // A document of its own: Markdown, rich text, saved.
    const tabs = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="tabs"] [role="tab"]')].map((tab) => tab.id),
    );
    await browser.execute(() => {
      document.getElementById('pw-explorer-list')?.focus();
    });
    await browser.keys(['Control', 'n']);
    await browser.waitUntil(
      () =>
        browser.execute(
          (before: string[]) =>
            [...document.querySelectorAll('[data-testid="tabs"] [role="tab"][aria-selected="true"]')].some(
              (tab) => !before.includes(tab.id),
            ),
          tabs,
        ),
      { timeoutMsg: 'Ctrl+N did not open a new document' },
    );
    await (await activeEditor()).click();
    await typeText('Barrierefrei bedienbar.');
    await browser.waitUntil(
      async () => (await $('[data-testid="status-save"]').getAttribute('data-state')) === 'saved',
      { timeout: 10_000, timeoutMsg: 'the text was not saved' },
    );
  });

  afterEach(async () => {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      const open = await browser.execute(
        () =>
          document.querySelector(
            '[data-testid="command-palette"], [data-testid="context-menu"], [role="dialog"]',
          ) !== null,
      );
      if (!open) {
        break;
      }
      await browser.keys('Escape');
      await browser.pause(200);
    }
    await closeSettings();
  });

  after(async () => {
    await setTheme(startTheme);
  });

  it('offers every registered command in the command palette, or knows why it is hidden', async () => {
    await focusEditor();
    await browser.keys(['Control', 'Shift', 'p']);
    await $('[data-testid="command-palette"]').waitForDisplayed();
    const listed = new Set(await paletteIds());
    const registered = registeredCommands();
    expect(registered.length).toBeGreaterThan(90);
    const missing = registered.filter((id) => !listed.has(id) && !(id in HIDDEN_HERE));
    expect(missing).toEqual([]);
    // Nothing is listed that is not registered, and no reason is stale.
    expect([...listed].filter((id) => !registered.includes(id))).toEqual([]);
    expect(Object.keys(HIDDEN_HERE).filter((id) => !registered.includes(id))).toEqual([]);
    measured(
      `Command palette (${process.platform}): ${String(listed.size)} of ${String(registered.length)} commands listed; hidden here: ${registered.filter((id) => !listed.has(id)).join(', ')}`,
    );
  });

  it('keeps the focus in the command palette, closes it with Escape and gives the focus back', async () => {
    const origin = await focusEditor();
    await browser.keys(['Control', 'Shift', 'p']);
    const palette = $('[data-testid="command-palette"]');
    await palette.waitForDisplayed();
    expect(await focused()).toContain('[command-palette]');
    await browser.keys('Tab');
    expect(await focused()).toContain('[command-palette]');
    await browser.keys('Escape');
    await expect(palette).not.toBeExisting();
    expect(await focused()).toBe(origin);
  });

  it('opens the dialogs by keyboard, keeps the focus inside, closes them with Escape and gives the focus back', async () => {
    const problems: string[] = [];
    for (const { command, dialog } of DIALOGS) {
      const origin = await focusEditor();
      await runByKeyboard(command);
      const selector = `[data-testid="${dialog}"]`;
      await $(selector).waitForDisplayed({ timeoutMsg: `${command} did not open ${dialog}` });
      await browser
        .waitUntil(
          () =>
            browser.execute(
              (target: string) => document.querySelector(target)?.contains(document.activeElement) === true,
              selector,
            ),
          { timeout: 3_000 },
        )
        .catch(() => {
          problems.push(`${dialog}: the focus did not move into the dialog`);
        });
      problems.push(...(await trapProblems(selector)));
      problems.push(...(await ariaProblems(selector)).map((problem) => `${dialog}: ${problem}`));
      // A lost Escape (F-27) would leave the dialog open, and its focus trap
      // would then take the keys of the next palette: press it again.
      let closed = false;
      for (let press = 0; press < 3 && !closed; press += 1) {
        await browser.keys('Escape');
        closed = await $(selector)
          .waitForExist({ reverse: true, timeout: 5_000 })
          .then(() => true)
          .catch(() => false);
      }
      if (!closed) {
        problems.push(`${dialog}: Escape did not close it`);
        break;
      }
      const back = await focused();
      if (back !== origin) {
        problems.push(`${dialog}: the focus went to ${back} instead of back to ${origin}`);
      }
    }
    expect(problems).toEqual([]);
  });

  it('reaches every section of the settings with the arrow keys', async () => {
    await runByKeyboard('settings.open');
    await $('[data-testid="settings-dialog"]').waitForDisplayed();
    await browser.execute(() => {
      document
        .querySelector<HTMLElement>('[data-testid="settings-dialog"] [role="tab"][aria-selected="true"]')
        ?.focus();
    });
    const first = await $('[data-testid="settings-dialog"] [role="tab"][aria-selected="true"]').getAttribute(
      'id',
    );
    const seen: string[] = [];
    const problems: string[] = [];
    for (let step = 0; step < SETTINGS_SECTIONS.length; step += 1) {
      const tab = await browser.execute(() => {
        const active = document.activeElement;
        return {
          id: active?.id ?? '',
          selected: active?.getAttribute('aria-selected') ?? '',
          panel:
            document
              .querySelector('[data-testid="settings-dialog"] [role="tabpanel"]')
              ?.getAttribute('aria-labelledby') ?? '',
        };
      });
      seen.push(tab.id.replace('settings-tab-', ''));
      if (tab.selected !== 'true' || tab.panel !== tab.id) {
        problems.push(`${tab.id}: selected ${tab.selected}, panel of ${tab.panel}`);
      }
      problems.push(
        ...(await ariaProblems('[data-testid="settings-dialog"]')).map((problem) => `${tab.id}: ${problem}`),
      );
      await browser.keys('ArrowRight');
    }
    expect([...seen].sort()).toEqual([...SETTINGS_SECTIONS].sort());
    // Around once: back at the first section.
    expect(await browser.execute(() => document.activeElement?.id ?? '')).toBe(first);
    expect(problems).toEqual([]);
  });

  it('moves the focus to the Explorer, the search and the writing surface with the keyboard', async () => {
    await focusEditor();
    await browser.keys(['Control', 'Shift', 'e']);
    await browser.waitUntil(async () => (await focused()).includes('#pw-explorer-list'), {
      timeoutMsg: 'Ctrl+Shift+E did not focus the Explorer',
    });
    await browser.keys(['Control', 'Shift', 'f']);
    await browser.waitUntil(
      () => browser.execute(() => document.activeElement?.closest('[data-testid="search"]') !== null),
      { timeoutMsg: 'Ctrl+Shift+F did not focus the search' },
    );
    await runByKeyboard('editor.focus');
    await browser.waitUntil(
      () => browser.execute(() => document.activeElement?.getAttribute('data-testid') === 'editor'),
      { timeoutMsg: '"Schreibfläche" did not focus the writing surface' },
    );
    // Back to the Explorer for the files after this one.
    await runByKeyboard('explorer.focus');
    await browser.waitUntil(async () => (await focused()).includes('#pw-explorer-list'));
  });

  it('opens the context menu of a tab with Shift+F10, moves in it and gives the focus back', async () => {
    await browser.execute(() => {
      document.querySelector<HTMLElement>('[data-testid="tabs"] [role="tab"][aria-selected="true"]')?.focus();
    });
    const origin = await focused();
    await browser.keys(['Shift', 'F10']);
    const menu = $('[data-testid="context-menu"]');
    await menu.waitForDisplayed({ timeoutMsg: 'Shift+F10 did not open the menu of the tab' });
    const firstItem = await focused();
    expect(firstItem).toContain('[context-menu]');
    await browser.keys('ArrowDown');
    expect(await focused()).toContain('[context-menu]');
    expect(await ariaProblems('[data-testid="context-menu"]')).toEqual([]);
    await browser.keys('Escape');
    await expect(menu).not.toBeExisting();
    expect(await focused()).toBe(origin);
  });

  it('follows the ARIA rules in the workbench, the search, the gallery and the Mesh', async () => {
    const problems: string[] = [];
    const check = async (label: string, scope = 'body') => {
      problems.push(...(await ariaProblems(scope)).map((problem) => `${label}: ${problem}`));
    };
    await check('workbench with a document');
    await runByKeyboard('find.show');
    await $('[role="tabpanel"]:not([hidden]) [data-testid="find-bar"]').waitForDisplayed();
    await check('find bar');
    await browser.keys('Escape');
    await browser.keys(['Control', 'Shift', 'p']);
    await $('[data-testid="command-palette"]').waitForDisplayed();
    await check('command palette', '[data-testid="command-palette"]');
    await browser.keys('Escape');
    await browser.keys(['Control', 'Shift', 'f']);
    await browser.keys('Kapitel'.split(''));
    await browser.pause(500);
    await check('search with hits');
    await runByKeyboard('explorer.focus');
    await runByKeyboard('media.gallery');
    await $('[data-testid="gallery"]').waitForDisplayed();
    await check('gallery');
    await browser.keys(['Control', 'w']);
    await browser.keys(['Control', 'Shift', 'm']);
    await $('[data-testid="mesh"]').waitForDisplayed();
    await browser.pause(1_000);
    await check('Mesh');
    await browser.keys(['Control', 'w']);
    await $('[data-testid="mesh"]').waitForExist({ reverse: true });
    expect(problems).toEqual([]);
  });

  for (const theme of ['dark', 'light'] as ThemeName[]) {
    it(`reaches 4.5:1 for text and 3:1 for icons and controls in the ${theme} theme`, async () => {
      await setTheme(theme);
      await focusEditor();
      const problems: string[] = [];
      problems.push(...(await contrastProblems(theme, 'body', 'workbench')));
      await browser.keys(['Control', 'Shift', 'p']);
      await $('[data-testid="command-palette"]').waitForDisplayed();
      problems.push(...(await contrastProblems(theme, '[data-testid="command-palette"]', 'command palette')));
      await browser.keys('Escape');
      for (const section of ['general', 'language', 'shortcuts', 'advanced']) {
        if (!(await $('[data-testid="settings-dialog"]').isExisting())) {
          await runByKeyboard('settings.open');
        }
        await $(`[data-testid="settings-tab-${section}"]`).click();
        problems.push(
          ...(await contrastProblems(theme, '[data-testid="settings-dialog"]', `settings ${section}`)),
        );
      }
      await closeSettings();
      await browser.execute(() => {
        document
          .querySelector<HTMLElement>('[data-testid="tabs"] [role="tab"][aria-selected="true"]')
          ?.focus();
      });
      await browser.keys(['Shift', 'F10']);
      await $('[data-testid="context-menu"]').waitForDisplayed();
      problems.push(...(await contrastProblems(theme, '[data-testid="context-menu"]', 'context menu', 2)));
      await browser.keys('Escape');
      measured(
        `Contrast, rendered (${process.platform}, ${theme} theme; target text 4.5, icons and controls 3): ${contrastSummary.splice(0).join('; ')}`,
      );
      expect(problems).toEqual([]);
    });
  }
});
