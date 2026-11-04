/**
 * Converting library documents in the running app (Phase 5, ADR 0015,
 * decision 13): Markdown becomes `.pwrite` without a question, `.pwrite`
 * becomes plain text only after the warning that the formatting goes. The
 * id stays; the file moves to the folder of its new format.
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
    await browser.waitUntil(() => fs.existsSync(documentFile(id, 'pwrite')), {
      timeout: 10_000,
      timeoutMsg: 'no .pwrite file after converting',
    });
    expect(fs.existsSync(documentFile(id, 'md'))).toBe(false);
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
    await browser.waitUntil(() => fs.existsSync(documentFile(id, 'txt')), {
      timeout: 10_000,
      timeoutMsg: 'no .txt file after converting',
    });
    expect(fs.readFileSync(documentFile(id, 'txt'), 'utf8')).toBe('Kapitel eins');
    expect(fs.existsSync(documentFile(id, 'pwrite'))).toBe(false);
    expect(await $('[data-testid="status-bar"]').getText()).toContain('Text');
  });
});
