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
 * Runs after external.e2e.ts in the same library.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { pickedFolders } from '../sandbox';
import { closeSettings, pinMainWindow } from '../session';

const FOLDERS = { md: 'markdown', pwrite: 'richtext', txt: 'text' } as const;

function documentFile(id: string, format: keyof typeof FOLDERS): string {
  return path.join(pickedFolders.moveTarget, 'documents', FOLDERS[format], `${id}.${format}`);
}

async function activeDocumentId(): Promise<string> {
  return (await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getAttribute('data-tab')) ?? '';
}

async function activeEditor() {
  for (const surface of await $$('[role="tabpanel"]:not([hidden]) [data-testid="editor"]').getElements()) {
    if (await surface.isDisplayed()) {
      return surface;
    }
  }
  throw new Error('no visible writing surface');
}

/** Types like the embedded WebDriver, without its legacy key codes (see editor.e2e.ts). */
async function typeText(text: string) {
  await browser.execute(async (keys: string) => {
    for (const character of keys) {
      const target = document.activeElement ?? document.body;
      const options = { key: character, bubbles: true, cancelable: true, composed: true };
      if (target.dispatchEvent(new KeyboardEvent('keydown', options))) {
        // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
        document.execCommand('insertText', false, character);
      }
      target.dispatchEvent(new KeyboardEvent('keyup', options));
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }, text);
}

async function newDocument(): Promise<string> {
  const before = await activeDocumentId();
  await browser.keys(['Control', 'n']);
  await browser.waitUntil(async () => {
    const id = await activeDocumentId();
    return id !== '' && id !== before && !id.startsWith('ext:');
  });
  return activeDocumentId();
}

/** Renames the active document from the palette ("Umbenennen", F2 in the Explorer). */
async function renameActive(title: string) {
  await runFromPalette('document.rename');
  const field = $('[data-testid="explorer-rename"]');
  await field.waitForDisplayed();
  await field.setValue(title);
  await browser.keys('Enter');
  await field.waitForExist({ reverse: true });
}

/** Runs a command from the command palette. */
async function runFromPalette(command: string) {
  await browser.keys(['Control', 'Shift', 'p']);
  const option = $(`[data-testid="command-palette"] [data-command="${command}"]`);
  await option.waitForDisplayed({ timeoutMsg: `${command} is not offered` });
  await option.click();
  await $('[data-testid="command-palette"]').waitForExist({ reverse: true });
}

describe('Converting documents (Phase 5)', () => {
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
    await renameActive('Linkziel');
    const source = await newDocument();
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
});
