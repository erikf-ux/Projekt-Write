/**
 * Library documents in the running app (Phase 5, ADR 0015):
 *
 * * Converting (decision 13): Markdown becomes `.pwrite` without a
 *   question, `.pwrite` becomes plain text only after the warning that the
 *   formatting goes. The id stays; the file moves to the folder of its new
 *   format.
 * * Internal links (decision 14): `[[` opens the document picker, the link
 *   is saved as `[Titel](pwrite://<id>)`, "Verbindungen" shows the
 *   backlink, Ctrl+click opens the linked document.
 *
 * Phase 6 (ADR 0016):
 *
 * * Stage 3: a deleted document goes to the trash and comes back with its
 *   connections.
 * * Stage 4: a named version, the comparison, and the restore that first
 *   keeps the current state as a version.
 *
 * Runs after external.e2e.ts in the same library.
 */
import fs from 'node:fs';
import { $, browser, expect } from '@wdio/globals';
import {
  activeDocumentId,
  activeEditor,
  appendText,
  documentFile,
  newDocument,
  renameActive,
  runFromPalette,
  typeText,
} from '../editing';
import { closeSettings, pinMainWindow } from '../session';

/** The row the Explorer has selected (its `aria-activedescendant`). */
async function explorerRow(): Promise<{ document: string; group: string }> {
  return browser.execute(() => {
    const tree = document.getElementById('pw-explorer-list');
    const row = document.getElementById(tree?.getAttribute('aria-activedescendant') ?? '');
    return {
      document: row?.getAttribute('data-document') ?? '',
      group: row?.getAttribute('data-group-member') ?? `group:${row?.getAttribute('data-group') ?? ''}`,
    };
  });
}

/** Focuses the Explorer, goes to its last row and up from there to the row wanted. */
async function selectInExplorer(document: string, group: string) {
  await browser.execute(() => {
    window.document.getElementById('pw-explorer-list')?.focus();
  });
  await browser.keys('End');
  for (let step = 0; step < 60; step += 1) {
    const row = await explorerRow();
    if (row.document === document && row.group === group) {
      return;
    }
    await browser.keys('ArrowUp');
  }
  throw new Error(`${document} is not in the Explorer group ${group}`);
}

function savedText(id: string): string | null {
  const file = documentFile(id, 'md');
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : null;
}

/** The label of the option a select of the history dialog shows. */
async function chosenVersion(testId: string): Promise<string> {
  return browser.execute((id: string) => {
    const select = document.querySelector<HTMLSelectElement>(`[data-testid="${id}"]`);
    return select?.selectedOptions[0]?.textContent ?? '';
  }, testId);
}

async function waitForSaved(id: string, text: string, message: string) {
  await browser.waitUntil(() => savedText(id) === text, { timeout: 10_000, timeoutMsg: message });
}

describe('Library documents (Phase 5 and 6)', () => {
  let linkTarget = '';
  let linkSource = '';

  before(async () => {
    await pinMainWindow();
    await closeSettings();
  });

  it('converts Markdown to .pwrite and .pwrite to text, keeping the id', async () => {
    await browser.keys(['Control', 'n']);
    await browser.waitUntil(
      async () => (await activeDocumentId()) !== '' && !(await activeDocumentId()).startsWith('ext:'),
    );
    const id = await activeDocumentId();
    await (await activeEditor()).click();
    await browser.keys('Kapitel eins'.split(''));
    await browser.waitUntil(
      () =>
        fs.existsSync(documentFile(id, 'md')) &&
        fs.readFileSync(documentFile(id, 'md'), 'utf8') === 'Kapitel eins\n',
      {
        timeout: 10_000,
        timeoutMsg: 'the Markdown document was not saved',
      },
    );

    await runFromPalette('document.convertToRichText');
    // Rust writes the new file, changes the row and removes the old file last.
    await browser.waitUntil(
      () => fs.existsSync(documentFile(id, 'pwrite')) && !fs.existsSync(documentFile(id, 'md')),
      { timeout: 10_000, timeoutMsg: 'no .pwrite file, or the .md file stayed, after converting' },
    );
    const envelope = JSON.parse(fs.readFileSync(documentFile(id, 'pwrite'), 'utf8')) as Record<
      string,
      unknown
    >;
    expect(envelope['id']).toBe(id);
    expect(JSON.stringify(envelope['content'])).toContain('Kapitel eins');
    expect(await $('[data-testid="status-bar"]').getText()).toContain('Rich Text (.pwrite)');
    expect(await activeDocumentId()).toBe(id);

    // Plain text loses the formatting: the app asks first.
    await runFromPalette('document.convertToText');
    const prompt = $('[data-testid="convert-prompt"]');
    await prompt.waitForDisplayed();
    expect(await prompt.getText()).toContain('Formatierung');
    await $('[data-testid="convert-prompt"] [data-choice="convert"]').click();
    await browser.waitUntil(
      () => fs.existsSync(documentFile(id, 'txt')) && !fs.existsSync(documentFile(id, 'pwrite')),
      { timeout: 10_000, timeoutMsg: 'no .txt file, or the .pwrite file stayed, after converting' },
    );
    expect(fs.readFileSync(documentFile(id, 'txt'), 'utf8')).toBe('Kapitel eins');
    expect(fs.existsSync(documentFile(id, 'pwrite'))).toBe(false);
    expect(await $('[data-testid="status-bar"]').getText()).toContain('Text');
  });

  it('links documents with [[, shows the backlink and opens the link with Ctrl+click', async () => {
    const target = await newDocument();
    linkTarget = target;
    await renameActive('Linkziel');
    const source = await newDocument();
    linkSource = source;
    await renameActive('Linkquelle');
    await (await activeEditor()).click();
    await typeText('Siehe [[');
    const picker = $('[data-testid="internal-link-picker"]');
    await picker.waitForDisplayed({ timeoutMsg: '[[ did not open the document picker' });
    await $('[data-testid="internal-link-search"]').setValue('Linkziel');
    await $(`[data-testid="internal-link-picker"] [data-document="${target}"]`).waitForDisplayed();
    await browser.keys('Enter');
    await picker.waitForExist({ reverse: true });
    await browser.waitUntil(
      () =>
        fs.existsSync(documentFile(source, 'md')) &&
        fs.readFileSync(documentFile(source, 'md'), 'utf8') === `Siehe [Linkziel](pwrite://${target})\n`,
      { timeout: 10_000, timeoutMsg: 'the internal link was not saved as Markdown' },
    );

    // Ctrl+click opens the linked document in the app; its connections show the source.
    await browser.execute((address: string) => {
      const anchor = document.querySelector<HTMLElement>(
        `[role="tabpanel"]:not([hidden]) [data-testid="editor"] a[href="${address}"]`,
      );
      if (!anchor) {
        throw new Error(`no link to ${address}`);
      }
      const box = anchor.getBoundingClientRect();
      const options = {
        bubbles: true,
        cancelable: true,
        view: window,
        button: 0,
        clientX: box.left + box.width / 2,
        clientY: box.top + box.height / 2,
        ctrlKey: true,
      };
      anchor.dispatchEvent(new MouseEvent('mousedown', { ...options, buttons: 1 }));
      anchor.dispatchEvent(new MouseEvent('mouseup', options));
      anchor.dispatchEvent(new MouseEvent('click', options));
    }, `pwrite://${target}`);
    await browser.waitUntil(async () => (await activeDocumentId()) === target, {
      timeoutMsg: 'Ctrl+click did not open the linked document',
    });
    const backlink = $(`[data-testid="connections"] [data-backlink="${source}"]`);
    await backlink.waitForDisplayed({ timeoutMsg: '"Verbindungen" does not show the backlink' });
    expect(await backlink.getText()).toBe('Linkquelle');
    await backlink.click();
    await browser.waitUntil(async () => (await activeDocumentId()) === source);
  });

  it('moves a document to the trash and back with its connections (stage 3)', async () => {
    await selectInExplorer(linkSource, 'unassigned');
    await browser.keys('Delete');
    await $(`[data-testid="tabs"] [data-tab="${linkSource}"]`).waitForExist({
      reverse: true,
      timeoutMsg: 'the deleted document stayed open',
    });
    // The trash keeps the file until it is emptied.
    expect(savedText(linkSource)).toBe(`Siehe [Linkziel](pwrite://${linkTarget})\n`);
    await $(`[data-testid="tabs"] [data-tab="${linkTarget}"]`).click();
    await browser.waitUntil(async () => (await activeDocumentId()) === linkTarget);
    await $('[data-testid="connections-empty"]').waitForDisplayed({
      timeoutMsg: 'a document in the trash still links here',
    });

    // The trash is the last group, closed at start.
    await browser.execute(() => {
      document.getElementById('pw-explorer-list')?.focus();
    });
    await browser.keys('End');
    expect(await explorerRow()).toEqual({ document: '', group: 'group:trash' });
    await browser.keys('ArrowRight');
    await browser.keys('End');
    expect(await explorerRow()).toEqual({ document: linkSource, group: 'trash' });
    await browser.keys('Enter');
    const restore = $('[data-testid="context-menu"] [data-item="restore"]');
    await restore.waitForDisplayed({ timeoutMsg: 'no menu for the document in the trash' });
    expect(await restore.getText()).toContain('Stufe 3');
    await restore.click();
    const backlink = $(`[data-testid="connections"] [data-backlink="${linkSource}"]`);
    await backlink.waitForDisplayed({ timeoutMsg: 'the restored document does not link here again' });
    expect(await backlink.getText()).toBe('Linkquelle');
    await expect($('[data-testid="explorer"] [data-group="trash"]')).toHaveText(expect.stringContaining('0'));
  });

  it('saves a named version, compares it and restores it, keeping the current state (stage 4)', async () => {
    const id = await newDocument();
    await (await activeEditor()).click();
    await typeText('Erste Fassung');
    await waitForSaved(id, 'Erste Fassung\n', 'the first text was not saved');
    await runFromPalette('document.saveVersion');
    const name = $('[data-testid="version-name"]');
    await name.waitForDisplayed();
    await name.setValue('Fassung 1');
    // The embedded WebDriver's synthetic Enter does not submit a form.
    await $('[data-testid="version-name-dialog"] button[type="submit"]').click();
    await $('[data-testid="version-name-dialog"]').waitForExist({ reverse: true });

    await appendText(' und mehr');
    await waitForSaved(id, 'Erste Fassung und mehr\n', 'the change was not saved');
    await runFromPalette('document.history');
    await $('[data-testid="history-dialog"]').waitForDisplayed();
    // The newest version against the current state: one line changed.
    expect(await chosenVersion('history-left')).toContain('Fassung 1');
    expect(await chosenVersion('history-right')).toBe('Aktueller Stand');
    await expect($('[data-testid="history-counts"]')).toHaveText('1 Zeilen entfernt, 1 hinzugefügt');
    await expect($('[data-testid="history-table"] [data-line="removed"]')).toHaveText(
      expect.stringContaining('Erste Fassung'),
    );
    await expect($('[data-testid="history-table"] [data-line="added"]')).toHaveText(
      expect.stringContaining('Erste Fassung und mehr'),
    );
    await $('[data-testid="history-restore"]').click();
    await $('[data-testid="history-dialog"]').waitForExist({ reverse: true });
    await waitForSaved(id, 'Erste Fassung\n', 'the version was not restored');
    await browser.waitUntil(async () => (await (await activeEditor()).getText()).trim() === 'Erste Fassung', {
      timeoutMsg: 'the editor does not show the restored text',
    });

    // The state before the restore is a version of its own.
    await runFromPalette('document.history');
    await $('[data-testid="history-dialog"]').waitForDisplayed();
    expect(await chosenVersion('history-left')).toContain('Vor einer Wiederherstellung');
    await expect($('[data-testid="history-table"] [data-line="removed"]')).toHaveText(
      expect.stringContaining('Erste Fassung und mehr'),
    );
    await browser.keys('Escape');
    await $('[data-testid="history-dialog"]').waitForExist({ reverse: true });
  });
});
