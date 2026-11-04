/**
 * Phase 4 in the running app (section 12): web links and formatting
 * shortcuts on the Tiptap writing surface, through to Rust and the file.
 *
 * Runs after workbench.e2e.ts in the same library. The e2e build records
 * opened links in `PW_E2E_OPENED_LINKS` instead of starting a browser
 * (ADR 0008); everything up to that call is the real path.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { openedLinks, pickedFolders } from '../sandbox';
import { closeSettings, pinMainWindow } from '../session';

function documentFile(id: string): string {
  return path.join(pickedFolders.moveTarget, 'documents', 'markdown', `${id}.md`);
}

function recordedLinks(): string[] {
  return fs.existsSync(openedLinks) ? fs.readFileSync(openedLinks, 'utf8').split('\n').filter(Boolean) : [];
}

async function activeDocumentId(): Promise<string> {
  return (await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getAttribute('data-tab')) ?? '';
}

async function tabIds(): Promise<string[]> {
  const ids: string[] = [];
  for (const tab of await $$('[data-testid="tabs"] [role="tab"]').getElements()) {
    ids.push((await tab.getAttribute('data-tab')) ?? '');
  }
  return ids;
}

async function saved(): Promise<void> {
  await browser.waitUntil(
    async () => (await $('[data-testid="status-save"]').getAttribute('data-state')) === 'saved',
    { timeout: 10_000, timeoutMsg: 'the text was not saved' },
  );
}

/** The visible writing surface of the active tab. */
async function activeEditor() {
  for (const surface of await $$('[role="tabpanel"]:not([hidden]) [data-testid="editor"]').getElements()) {
    if (await surface.isDisplayed()) {
      return surface;
    }
  }
  throw new Error('no visible writing surface');
}

/**
 * Types into the focused writing surface like the embedded WebDriver
 * (`keydown`, then `insertText`), but without its legacy `keyCode`: it
 * sends the character code, so `.` arrives as 46, which ProseMirror reads
 * as the Delete key. A keyboard sends 190 for `.`.
 */
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

/** A mouse click on the link with this address, as the system sends it (mousedown, mouseup, click). */
async function clickLink(href: string, ctrlKey: boolean) {
  await browser.execute(
    (address: string, withCtrl: boolean) => {
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
        ctrlKey: withCtrl,
      };
      anchor.dispatchEvent(new MouseEvent('mousedown', { ...options, buttons: 1 }));
      anchor.dispatchEvent(new MouseEvent('mouseup', options));
      anchor.dispatchEvent(new MouseEvent('click', options));
    },
    href,
    ctrlKey,
  );
}

describe('Project Write – Phase 4 editor', () => {
  let id = '';

  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    await closeSettings();
    // The app may start with the tabs of the last session: wait for the new one.
    const before = await tabIds();
    await $('[data-testid="explorer"] [role="listbox"]').click();
    await browser.keys(['Control', 'n']);
    await browser.waitUntil(
      async () => {
        const active = await activeDocumentId();
        return active !== '' && !before.includes(active);
      },
      { timeoutMsg: 'Ctrl+N did not open a new document' },
    );
    id = await activeDocumentId();
    const editor = await activeEditor();
    await editor.waitForDisplayed();
    await editor.click();
    // A web address becomes a link when the space after it is typed.
    await typeText('Erst https://example.org/eins dann https://example.org/zwei Ende');
    await saved();
  });

  it('turns typed web addresses into links and saves them as Markdown', () => {
    expect(fs.readFileSync(documentFile(id), 'utf8')).toBe(
      'Erst <https://example.org/eins> dann <https://example.org/zwei> Ende\n',
    );
  });

  it('opens a link with Ctrl+click only, through Rust', async () => {
    const before = recordedLinks().length;
    await clickLink('https://example.org/zwei', false);
    await clickLink('https://example.org/eins', true);
    await browser.waitUntil(() => recordedLinks().length > before, {
      timeoutMsg: 'Ctrl+click did not reach link_open',
    });
    expect(recordedLinks().slice(before)).toEqual(['https://example.org/eins']);
    // The webview itself followed no link: the app is still on its page.
    await expect($('[data-testid="workbench"]')).toBeDisplayed();
  });

  it('formats the selection with Ctrl+B and saves the Markdown', async () => {
    await browser.execute(() => {
      const paragraph = document.querySelector('[role="tabpanel"]:not([hidden]) [data-testid="editor"] p');
      const text = paragraph?.firstChild;
      if (!text || text.nodeType !== Node.TEXT_NODE) {
        throw new Error('no text at the start of the paragraph');
      }
      const range = document.createRange();
      range.setStart(text, 0);
      range.setEnd(text, 'Erst'.length);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    });
    await browser.waitUntil(
      async () => (await browser.execute(() => window.getSelection()?.toString() ?? '')) === 'Erst',
    );
    await browser.keys(['Control', 'b']);
    await browser.waitUntil(() => fs.readFileSync(documentFile(id), 'utf8').startsWith('**Erst**'), {
      timeout: 10_000,
      timeoutMsg: 'Ctrl+B did not reach the saved file',
    });
    expect(fs.readFileSync(documentFile(id), 'utf8')).toBe(
      '**Erst** <https://example.org/eins> dann <https://example.org/zwei> Ende\n',
    );
  });
});
