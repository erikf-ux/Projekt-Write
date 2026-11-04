/**
 * External files in the running app (Phase 5, ADR 0015): opened with
 * Ctrl+O through the file dialog of Rust, edited and saved with Ctrl+S.
 *
 * * UTF-8, UTF-8 with BOM, UTF-16 LE/BE and Windows-1252 with LF or CRLF
 *   keep their encoding and line endings when edited.
 * * An unchanged file stays byte-identical, even after Ctrl+S.
 * * A change from outside opens the dialog: "Neu laden" shows the file's
 *   text, "Meine Fassung behalten" writes the tab's text over it.
 * * Closing a tab with changes asks; "Verwerfen" leaves the file as it was.
 * * A second start of the app with a file (what a double-click or "Öffnen
 *   mit" does while the app runs) opens it in the running app and ends.
 *
 * The e2e build answers the file dialog from `PW_E2E_PICK_FILES`
 * (`pickedFiles` in sandbox.ts, ADR 0008); the files lie in the test folder.
 * Runs last, in the library the earlier files left.
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { application, pickedFiles, secondStartFile } from '../sandbox';
import { closeSettings, pinMainWindow } from '../session';

const BOM_UTF8 = Buffer.from([0xef, 0xbb, 0xbf]);
const BOM_UTF16LE = Buffer.from([0xff, 0xfe]);
const BOM_UTF16BE = Buffer.from([0xfe, 0xff]);

function utf16be(text: string): Buffer {
  const bytes = Buffer.from(text, 'utf16le');
  for (let index = 0; index + 1 < bytes.length; index += 2) {
    const low = bytes[index] ?? 0;
    bytes[index] = bytes[index + 1] ?? 0;
    bytes[index + 1] = low;
  }
  return bytes;
}

/** Windows-1252 for Latin-1 text plus the en dash and the euro sign. */
function windows1252(text: string): Buffer {
  const special: Record<string, number> = { '–': 0x96, '€': 0x80 };
  return Buffer.from(
    Array.from(text, (character) => {
      const code = special[character] ?? character.charCodeAt(0);
      if (code > 0xff || (code >= 0x80 && code <= 0x9f && !(character in special))) {
        throw new Error(`not in the test table: ${character}`);
      }
      return code;
    }),
  );
}

interface Sample {
  file: string;
  encoding: string;
  lineEnding: 'lf' | 'crlf';
  text: string;
  bytes: (text: string) => Buffer;
}

const crlf = (text: string) => text.replace(/\n/g, '\r\n');

const SAMPLES: Sample[] = [
  {
    file: pickedFiles.utf8,
    encoding: 'utf8',
    lineEnding: 'lf',
    text: 'Erste Zeile\nZweite Zeile',
    bytes: (text) => Buffer.from(text, 'utf8'),
  },
  {
    file: pickedFiles.utf8Bom,
    encoding: 'utf8Bom',
    lineEnding: 'crlf',
    text: 'Größe\nMaß',
    bytes: (text) => Buffer.concat([BOM_UTF8, Buffer.from(crlf(text), 'utf8')]),
  },
  {
    file: pickedFiles.utf16le,
    encoding: 'utf16Le',
    lineEnding: 'crlf',
    text: 'Grüße\nausUTF-16',
    bytes: (text) => Buffer.concat([BOM_UTF16LE, Buffer.from(crlf(text), 'utf16le')]),
  },
  {
    file: pickedFiles.utf16be,
    encoding: 'utf16Be',
    lineEnding: 'lf',
    text: 'Zeile eins\nZeile zwei',
    bytes: (text) => Buffer.concat([BOM_UTF16BE, utf16be(text)]),
  },
  {
    file: pickedFiles.windows1252,
    encoding: 'windows1252',
    lineEnding: 'crlf',
    text: 'Straße – Maß\nÄrger für 5 €',
    bytes: (text) => windows1252(crlf(text)),
  },
];

/** The tab of an external file: its tooltip starts with the file name. */
const tabSelector = (name: string) =>
  `[data-testid="tabs"] [role="tab"][data-tab^="ext:"][title^="${name} "]`;

/** Types into the focused writing surface like the embedded WebDriver (see editor.e2e.ts). */
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

/** The visible writing surface of the active tab. */
async function activeEditor() {
  for (const surface of await $$('[role="tabpanel"]:not([hidden]) [data-testid="editor"]').getElements()) {
    if (await surface.isDisplayed()) {
      return surface;
    }
  }
  throw new Error('no visible writing surface');
}

/** Ctrl+O: the dialog answers with the next file of `pickedFiles`. */
async function openNext(file: string) {
  const name = path.basename(file);
  await browser.keys(['Control', 'o']);
  const tab = $(tabSelector(name));
  await tab.waitForExist({ timeout: 20_000, timeoutMsg: `${name} did not open` });
  await browser.waitUntil(async () => (await tab.getAttribute('aria-selected')) === 'true');
  await (await activeEditor()).waitForDisplayed();
  return tab;
}

/**
 * Appends text at the end of the active document. The caret goes there
 * through the DOM selection, which ProseMirror follows; WebKitGTK's
 * WebDriver does not move it with Ctrl+End.
 */
async function appendText(text: string) {
  await (await activeEditor()).click();
  await browser.execute(() => {
    const editor = document.querySelector<HTMLElement>(
      '[role="tabpanel"]:not([hidden]) [data-testid="editor"]',
    );
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

async function saveAndWait(file: string, expected: Buffer) {
  await browser.keys(['Control', 's']);
  await browser.waitUntil(() => fs.readFileSync(file).equals(expected), {
    timeout: 10_000,
    timeoutMsg: `${path.basename(file)} was not written as expected`,
  });
  await browser.waitUntil(
    async () => (await $(tabSelector(path.basename(file))).getAttribute('data-unsaved')) === null,
  );
}

async function closeActiveTab() {
  await browser.keys(['Control', 'w']);
}

describe('External files (Phase 5)', () => {
  before(async () => {
    await pinMainWindow();
    await closeSettings();
    fs.mkdirSync(path.dirname(pickedFiles.utf8), { recursive: true });
    for (const sample of SAMPLES) {
      fs.writeFileSync(sample.file, sample.bytes(sample.text));
    }
    fs.writeFileSync(pickedFiles.unchanged, Buffer.concat([BOM_UTF16BE, utf16be('Bleibt\r\nwie es ist')]));
    fs.writeFileSync(pickedFiles.outside, 'Vorher');
    fs.writeFileSync(pickedFiles.draft, 'Entwurf');
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 30_000 });
  });

  it('keeps encoding and line endings of every kind while editing', async () => {
    for (const sample of SAMPLES) {
      await openNext(sample.file);
      expect(await $('[data-testid="status-encoding"]').getValue()).toBe(sample.encoding);
      expect(await $('[data-testid="status-line-ending"]').getValue()).toBe(sample.lineEnding);
      await appendText(' Ende');
      await browser.waitUntil(
        async () =>
          (await $(tabSelector(path.basename(sample.file))).getAttribute('data-unsaved')) === 'true',
        { timeoutMsg: 'the tab does not show its change' },
      );
      // Not saved before Ctrl+S.
      expect(fs.readFileSync(sample.file).equals(sample.bytes(sample.text))).toBe(true);
      await saveAndWait(sample.file, sample.bytes(`${sample.text} Ende`));
      await closeActiveTab();
      await $(tabSelector(path.basename(sample.file))).waitForExist({ reverse: true });
    }
  });

  it('leaves an unchanged file byte-identical, even after Ctrl+S', async () => {
    const before = fs.readFileSync(pickedFiles.unchanged);
    const modified = fs.statSync(pickedFiles.unchanged).mtimeMs;
    await openNext(pickedFiles.unchanged);
    expect(await $('[data-testid="status-encoding"]').getValue()).toBe('utf16Be');
    await browser.keys(['Control', 's']);
    await browser.pause(1500);
    await closeActiveTab();
    await $(tabSelector(path.basename(pickedFiles.unchanged))).waitForExist({ reverse: true });
    expect(await $('[data-testid="unsaved-prompt"]').isExisting()).toBe(false);
    expect(fs.readFileSync(pickedFiles.unchanged).equals(before)).toBe(true);
    expect(fs.statSync(pickedFiles.unchanged).mtimeMs).toBe(modified);
  });

  it('asks after a change from outside: reload, or keep my version', async () => {
    await openNext(pickedFiles.outside);
    fs.writeFileSync(pickedFiles.outside, 'Von außen');
    // A new element each time: the dialog of the second change is another one.
    const dialog = () => $('[data-testid="outside-change"]');
    await dialog().waitForDisplayed({
      timeout: 10_000,
      timeoutMsg: 'no dialog after the change from outside',
    });
    await $('[data-testid="outside-change"] [data-choice="reload"]').click();
    await dialog().waitForExist({ reverse: true });
    await browser.waitUntil(async () => (await (await activeEditor()).getText()).includes('Von außen'), {
      timeoutMsg: 'the reloaded text is not shown',
    });

    await appendText(', meine Fassung');
    fs.writeFileSync(pickedFiles.outside, 'Zweite Änderung von außen');
    await dialog().waitForDisplayed({ timeout: 10_000 });
    await $('[data-testid="outside-change"] [data-choice="keepMine"]').click();
    await dialog().waitForExist({ reverse: true });
    await saveAndWait(pickedFiles.outside, Buffer.from('Von außen, meine Fassung', 'utf8'));
    await closeActiveTab();
  });

  it('asks before closing a tab with changes; Discard leaves the file alone', async () => {
    await openNext(pickedFiles.draft);
    await appendText(' verworfen');
    const prompt = () => $('[data-testid="unsaved-prompt"]');
    await closeActiveTab();
    await prompt().waitForDisplayed();
    await $('[data-testid="unsaved-prompt"] [data-choice="cancel"]').click();
    await prompt().waitForExist({ reverse: true });
    expect(await $(tabSelector(path.basename(pickedFiles.draft))).isExisting()).toBe(true);

    await closeActiveTab();
    await prompt().waitForDisplayed();
    await $('[data-testid="unsaved-prompt"] [data-choice="discard"]').click();
    await $(tabSelector(path.basename(pickedFiles.draft))).waitForExist({ reverse: true });
    expect(fs.readFileSync(pickedFiles.draft, 'utf8')).toBe('Entwurf');
  });

  it('opens a file passed to a second start in the running app', async () => {
    fs.writeFileSync(secondStartFile, '# Aus dem zweiten Start\n');
    // The second instance hands the file over and ends at once.
    const second = spawnSync(application, [secondStartFile], { env: process.env, timeout: 60_000 });
    expect(second.error).toBeUndefined();
    expect(second.status).toBe(0);
    const tab = $(tabSelector(path.basename(secondStartFile)));
    await tab.waitForExist({ timeout: 20_000, timeoutMsg: 'the file of the second start did not open' });
    await browser.waitUntil(async () => (await tab.getAttribute('aria-selected')) === 'true');
    await browser.waitUntil(async () =>
      (await (await activeEditor()).getText()).includes('Aus dem zweiten Start'),
    );
    await closeActiveTab();
    await tab.waitForExist({ reverse: true });
  });

  it('lists the files under "Zuletzt verwendet"', async () => {
    const recent = $$('[data-recent-file]');
    await browser.waitUntil(async () => (await recent.length) >= 5, {
      timeoutMsg: 'external files are missing in "Zuletzt verwendet"',
    });
  });
});
