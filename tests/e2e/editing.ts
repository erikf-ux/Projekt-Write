/**
 * Writing helpers shared by the end-to-end tests of Phase 5 and 6.
 */
import path from 'node:path';
import { $, $$, browser } from '@wdio/globals';
import { pickedFolders } from './sandbox';

const FOLDERS = { md: 'markdown', pwrite: 'richtext', txt: 'text' } as const;

/** The library the tests from Phase 3 on work in (moved there in library.e2e.ts). */
export const libraryRoot = pickedFolders.moveTarget;

/** Where the library keeps a document of this format. */
export function documentFile(id: string, format: keyof typeof FOLDERS): string {
  return path.join(libraryRoot, 'documents', FOLDERS[format], `${id}.${format}`);
}

/** The draft of a library document (stage 2). */
export function draftFile(id: string): string {
  return path.join(libraryRoot, 'recovery', `${id}.json`);
}

export async function activeDocumentId(): Promise<string> {
  return (await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getAttribute('data-tab')) ?? '';
}

export async function activeEditor() {
  for (const surface of await $$('[role="tabpanel"]:not([hidden]) [data-testid="editor"]').getElements()) {
    if (await surface.isDisplayed()) {
      return surface;
    }
  }
  throw new Error('no visible writing surface');
}

/** Types like the embedded WebDriver, without its legacy key codes (see editor.e2e.ts). */
export async function typeText(text: string): Promise<void> {
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

/**
 * Puts the caret at the end of the active document and types there; the
 * embedded WebDriver does not move it with Ctrl+End.
 */
export async function appendText(text: string): Promise<void> {
  await (await activeEditor()).click();
  await browser.execute(() => {
    const editor = [
      ...document.querySelectorAll<HTMLElement>('[role="tabpanel"]:not([hidden]) [data-testid="editor"]'),
    ].find((element) => element.offsetParent !== null);
    if (!editor) {
      throw new Error('no writing surface');
    }
    editor.focus();
    const range = document.createRange();
    range.selectNodeContents(editor);
    range.collapse(false);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  });
  await browser.pause(100);
  await typeText(text);
}

/** Opens a new document with Ctrl+N; resolves with its id. */
export async function newDocument(): Promise<string> {
  const before = await activeDocumentId();
  await browser.keys(['Control', 'n']);
  await browser.waitUntil(async () => {
    const id = await activeDocumentId();
    return id !== '' && id !== before && !id.startsWith('ext:');
  });
  return activeDocumentId();
}

/** Runs a command from the command palette. */
export async function runFromPalette(command: string): Promise<void> {
  await browser.keys(['Control', 'Shift', 'p']);
  const option = $(`[data-testid="command-palette"] [data-command="${command}"]`);
  await option.waitForDisplayed({ timeoutMsg: `${command} is not offered` });
  await option.click();
  await $('[data-testid="command-palette"]').waitForExist({ reverse: true });
}

/** Renames the active document from the palette ("Umbenennen", F2 in the Explorer). */
export async function renameActive(title: string): Promise<void> {
  await runFromPalette('document.rename');
  const field = $('[data-testid="explorer-rename"]');
  await field.waitForDisplayed();
  await field.setValue(title);
  await browser.keys('Enter');
  await field.waitForExist({ reverse: true });
}
