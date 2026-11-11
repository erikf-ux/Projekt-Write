/**
 * The global search in the running app (Phase 9, section 19, ADR 0020).
 * The acceptance criteria of section 25:
 *
 * * `Muller` and `Mueller` find `Müller`, `Strasse` finds `Straße`, `haus`
 *   finds `Krankenhaus` and `Häuser`, `C++` and a lone `"` raise no error;
 * * archived elements appear only through the filter;
 * * the index is right after editing, deleting and restoring.
 *
 * Besides: Ctrl+Shift+F, the side bar that switches back to the Explorer,
 * opening a hit at the place it was found (find bar), keyboard navigation.
 * The first hits with the reference library are measured in
 * reference.e2e.ts. Runs after media.e2e.ts in the same library.
 */
import { $, $$, browser, expect } from '@wdio/globals';
import {
  activeDocumentId,
  appendText,
  documentFile,
  newDocument,
  renameActive,
  runFromPalette,
} from '../editing';
import fs from 'node:fs';
import { pinMainWindow } from '../session';

interface Hit {
  id: string;
  kind: string;
  match: string;
  title: string;
  archived: boolean;
  deleted: boolean;
  marked: string[];
}

/** Types `query` into the search field and waits until its hits are shown. */
async function search(query: string): Promise<Hit[]> {
  const shownBefore = await browser.execute(() => performance.getEntriesByName('pw:search-shown').length);
  await browser.execute((text: string) => {
    const field = document.querySelector<HTMLInputElement>('[data-testid="search-field"]');
    if (!field) {
      throw new Error('no search field');
    }
    field.focus();
    field.select();
    // Typed the way the WebDriver types (see editing.ts); an empty text
    // deletes the selection.
    if (text === '') {
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
      document.execCommand('delete');
    } else {
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
      document.execCommand('insertText', false, text);
    }
  }, query);
  await browser.waitUntil(
    async () =>
      (await browser.execute(() => performance.getEntriesByName('pw:search-shown').length)) > shownBefore,
    { timeout: 10_000, timeoutMsg: `no hits shown for ${query}` },
  );
  return hits();
}

async function hits(): Promise<Hit[]> {
  return browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>('[data-testid="search-results"] [data-hit]')].map((row) => ({
      id: row.getAttribute('data-hit') ?? '',
      kind: row.getAttribute('data-kind') ?? '',
      match: row.getAttribute('data-match') ?? '',
      title: row.querySelector('[data-title]')?.textContent ?? '',
      archived: row.querySelector('[aria-label="Im Archiv"]') !== null,
      deleted: row.querySelector('[aria-label="Im Papierkorb"]') !== null,
      marked: [...row.querySelectorAll('mark')].map((mark) => mark.textContent),
    })),
  );
}

async function status(): Promise<string> {
  return (await $('[data-testid="search-status"]').getAttribute('data-status')) ?? '';
}

async function createDocument(title: string, text: string): Promise<string> {
  const id = await newDocument();
  await appendText(text);
  await renameActive(title);
  await browser.waitUntil(
    () =>
      fs.existsSync(documentFile(id, 'md')) && fs.readFileSync(documentFile(id, 'md'), 'utf8').includes(text),
    {
      timeout: 10_000,
      timeoutMsg: `${title} was not saved`,
    },
  );
  return id;
}

async function openSearch(): Promise<void> {
  await browser.keys(['Control', 'Shift', 'f']);
  await $('[data-testid="search-field"]').waitForDisplayed({ timeoutMsg: 'Ctrl+Shift+F shows no search' });
  await browser.waitUntil(
    () => browser.execute(() => document.activeElement?.getAttribute('data-testid') === 'search-field'),
    { timeoutMsg: 'the search field has no focus' },
  );
}

async function setFilter(testId: string, checked: boolean): Promise<void> {
  if (!(await $('[data-testid="search-filters"]').isExisting())) {
    await $('[data-testid="search-filter-toggle"]').click();
  }
  const box = $(`[data-testid="${testId}"]`);
  await box.waitForDisplayed();
  if ((await box.isSelected()) !== checked) {
    const shownBefore = await browser.execute(() => performance.getEntriesByName('pw:search-shown').length);
    await box.click();
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => performance.getEntriesByName('pw:search-shown').length)) > shownBefore,
      { timeoutMsg: `no new hits after ${testId}` },
    );
  }
}

describe('Global search (Phase 9)', () => {
  let letter = '';
  let roster = '';
  let oldTown = '';
  let code = '';
  let garden = '';

  before(async () => {
    await pinMainWindow();
    letter = await createDocument('Brief', 'Sehr geehrter Herr Müller, wir wohnen in der Hauptstraße 5.');
    roster = await createDocument('Dienstplan', 'Schicht im Krankenhaus am Montag.');
    oldTown = await createDocument('Altstadt', 'Alte Häuser am Markt.');
    code = await createDocument('Programmieren', 'Code in C++ und Rust.');
    garden = await createDocument('Gartenarchiv', 'Gartenplanung von früher.');
    await runFromPalette('document.archive');
  });

  it('opens with Ctrl+Shift+F and finds spellings and parts of words', async () => {
    await openSearch();
    for (const query of ['Muller', 'Mueller', 'Müller']) {
      const found = await search(query);
      expect(found.map((hit) => hit.id)).toContain(letter);
      expect(found.find((hit) => hit.id === letter)?.marked).toContain('Müller');
    }
    expect((await search('Strasse')).map((hit) => hit.id)).toContain(letter);

    const parts = await search('haus');
    const ids = parts.map((hit) => hit.id);
    expect(ids).toContain(roster);
    expect(ids).toContain(oldTown);
    // "Häuser" begins with the word, "Krankenhaus" contains it: words first.
    expect(ids.indexOf(oldTown)).toBeLessThan(ids.indexOf(roster));
    expect(parts.find((hit) => hit.id === roster)?.match).toBe('parts');
  });

  it('takes C++ and a lone quote without an error', async () => {
    const cpp = await search('C++');
    expect(cpp.map((hit) => hit.id)).toContain(code);
    expect(await status()).toBe('done');
    expect(await search('"')).toEqual([]);
    expect(await status()).toBe('done');
    expect(await $('[data-testid="search-status"]').getText()).toBe('Keine Treffer');
  });

  it('shows archived documents only through the filter', async () => {
    expect((await search('Gartenplanung')).map((hit) => hit.id)).not.toContain(garden);
    await setFilter('search-filter-archive', true);
    const found = await hits();
    expect(found.map((hit) => hit.id)).toContain(garden);
    expect(found.find((hit) => hit.id === garden)?.archived).toBe(true);
    await setFilter('search-filter-archive', false);
    expect((await hits()).map((hit) => hit.id)).not.toContain(garden);
    await $('[data-testid="search-filter-toggle"]').click();
  });

  it('opens a hit at the place it was found and moves through the hits by keyboard', async () => {
    const found = await search('Krankenhaus');
    expect(found[0]?.id).toBe(roster);
    await browser.keys('Enter');
    await browser.waitUntil(async () => (await activeDocumentId()) === roster, {
      timeoutMsg: 'the hit did not open',
    });
    const term = $('[role="tabpanel"]:not([hidden]) [data-testid="find-term"]');
    await term.waitForDisplayed({ timeoutMsg: 'no find bar at the place found' });
    expect(await term.getValue()).toBe('Krankenhaus');
    await expect($('[role="tabpanel"]:not([hidden]) [data-testid="find-count"]')).toHaveText(
      expect.stringContaining('1'),
    );

    // Arrow keys in the field move the selection.
    await openSearch();
    await search('am');
    const before = await $('[data-testid="search-results"] [aria-selected="true"]').getAttribute('data-hit');
    await browser.keys('ArrowDown');
    const after = await $('[data-testid="search-results"] [aria-selected="true"]').getAttribute('data-hit');
    expect(after).not.toBe(before);

    // The activity bar goes back to the Explorer.
    await runFromPalette('explorer.focus');
    await $('[data-testid="explorer"]').waitForDisplayed();
    expect(await $('[data-testid="search"]').isExisting()).toBe(false);
  });

  it('keeps the index right after editing, deleting and restoring', async () => {
    // Editing: the roster is open; a new word.
    await appendText(' Leuchtturm');
    await browser.waitUntil(
      () => fs.readFileSync(documentFile(roster, 'md'), 'utf8').includes('Leuchtturm'),
      {
        timeout: 10_000,
        timeoutMsg: 'the edit was not saved',
      },
    );
    await openSearch();
    expect((await search('Leuchtturm')).map((hit) => hit.id)).toEqual([roster]);

    // Deleting: in the trash only through the filter.
    await $(`[data-testid="tabs"] [data-tab="${oldTown}"]`).click();
    await browser.waitUntil(async () => (await activeDocumentId()) === oldTown);
    await runFromPalette('document.delete');
    await $(`[data-testid="tabs"] [data-tab="${oldTown}"]`).waitForExist({ reverse: true });
    await openSearch();
    expect((await search('Häuser')).map((hit) => hit.id)).not.toContain(oldTown);
    await setFilter('search-filter-trash', true);
    const trashed = (await hits()).find((hit) => hit.id === oldTown);
    expect(trashed?.deleted).toBe(true);

    // A hit in the trash is shown in the Explorer; restored from there.
    await $(`[data-testid="search-results"] [data-hit="${oldTown}"]`).click();
    await $('[data-testid="explorer"]').waitForDisplayed();
    await browser.waitUntil(
      () =>
        browser.execute((id: string) => {
          const tree = document.getElementById('pw-explorer-list');
          const row = document.getElementById(tree?.getAttribute('aria-activedescendant') ?? '');
          return (
            row?.getAttribute('data-document') === id && row.getAttribute('data-group-member') === 'trash'
          );
        }, oldTown),
      { timeoutMsg: 'the trashed hit is not selected in the Explorer' },
    );
    await browser.execute(() => {
      document.getElementById('pw-explorer-list')?.focus();
    });
    await browser.keys('Enter');
    const restore = $('[data-testid="context-menu"] [data-item="restore"]');
    await restore.waitForDisplayed({ timeoutMsg: 'no menu for the document in the trash' });
    await restore.click();

    await openSearch();
    // Turning the filter off searches "Häuser" again, with the new state.
    await setFilter('search-filter-trash', false);
    const back = await hits();
    expect(back.find((hit) => hit.id === oldTown)?.deleted).toBe(false);
    await $('[data-testid="search-filter-toggle"]').click();
    await runFromPalette('explorer.focus');
  });

  it('rebuilds the index from the settings', async () => {
    await runFromPalette('search.rebuild');
    await expect($('[data-testid="notice"]')).toHaveText(expect.stringContaining('Suchindex neu aufgebaut'));
    await openSearch();
    expect((await search('Leuchtturm')).map((hit) => hit.id)).toEqual([roster]);
    expect(await $$('[data-testid="search-results"] [data-hit]').getElements()).toHaveLength(1);
    await runFromPalette('explorer.focus');
  });
});
