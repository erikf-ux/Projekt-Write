/**
 * Media in the running app (Phase 8, ADR 0019). The acceptance criteria of
 * section 25:
 *
 * * originals stay byte for byte the same (checksums before and after);
 * * an aborted import leaves neither files nor rows;
 * * an SVG with script runs nothing; a test PDF after the pattern of
 *   CVE-2024-4367 runs nothing.
 *
 * Besides: folder import with subnets, refused files with their reason,
 * duplicates, the gallery, the image and the PDF view.
 *
 * The import dialogs answer from `PW_E2E_PICK_MEDIA` (see sandbox.ts).
 * Runs after structure.e2e.ts in the same library.
 */
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { libraryRoot, runFromPalette } from '../editing';
import { cvePdf, docx, pdf, png, SCRIPT_SVG } from '../fixtures';
import { mediaOriginals, pickedMedia } from '../sandbox';
import { pinMainWindow } from '../session';

function sha256(file: string): string {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

/** Every file below `dir` with its checksum and modification time. */
function snapshot(dir: string): Map<string, { sha: string; mtime: number }> {
  const files = new Map<string, { sha: string; mtime: number }>();
  for (const entry of fs.readdirSync(dir, { recursive: true, withFileTypes: true })) {
    if (entry.isFile()) {
      const file = path.join(entry.parentPath, entry.name);
      files.set(path.relative(dir, file), { sha: sha256(file), mtime: fs.statSync(file).mtimeMs });
    }
  }
  return files;
}

function write(file: string, content: Buffer | string) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
}

/** Files in the media folders of the library. */
function mediaFiles(): string[] {
  const found: string[] = [];
  for (const kind of ['images', 'pdf', 'attachments']) {
    const dir = path.join(libraryRoot, 'media', kind);
    if (fs.existsSync(dir)) {
      found.push(...fs.readdirSync(dir).map((name) => `${kind}/${name}`));
    }
  }
  return found.sort();
}

function importLeftovers(): string[] {
  const dir = path.join(libraryRoot, 'tmp');
  return fs.existsSync(dir) ? fs.readdirSync(dir).filter((name) => name.startsWith('import-')) : [];
}

/** The tab panel in front; the other media tabs stay in the DOM, hidden. */
const SHOWN = '[role="tabpanel"]:not([hidden])';

async function galleryTiles() {
  return $$('[data-testid="gallery-list"] [data-media]').getElements();
}

async function openTile(kind: string, title: string) {
  const tile = $(
    `//*[@data-testid="gallery-list"]//*[@data-media-kind="${kind}"][.//span[normalize-space()="${title}"]]`,
  );
  await tile.waitForDisplayed();
  // Select it and open it with the keyboard (Enter in the list box); a
  // WebDriver double click does not reach WebKitGTK as one dblclick.
  await tile.click();
  await browser.waitUntil(async () => (await tile.getAttribute('aria-selected')) === 'true');
  await browser.keys('Enter');
  await $(`${SHOWN} [data-testid="media-view"][data-kind="${kind}"]`).waitForDisplayed();
}

/** Markers the hostile files try to set. */
async function markers() {
  return browser.execute(() => ({
    svg: (window as unknown as { __pwSvg?: number }).__pwSvg ?? null,
    cve: (window as unknown as { __pwCve?: number }).__pwCve ?? null,
    title: document.title,
  }));
}

const originals = {
  ankunft: path.join(pickedMedia.folder, 'Ankunft.png'),
  skript: path.join(pickedMedia.folder, 'Skript.svg'),
  studie: path.join(pickedMedia.folder, 'Tag 1', 'Studie.pdf'),
  cve: path.join(pickedMedia.folder, 'Tag 1', 'cve-2024-4367.pdf'),
  brief: path.join(pickedMedia.folder, 'Tag 1', 'Abend', 'Brief.docx'),
};

describe('Media (Phase 8)', () => {
  let originalsBefore: Map<string, { sha: string; mtime: number }>;
  let titleBefore: string;

  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    fs.rmSync(mediaOriginals, { recursive: true, force: true });
    write(originals.ankunft, png(320, 200, [40, 120, 200]));
    write(originals.skript, SCRIPT_SVG);
    write(originals.studie, pdf(3, 'Studie Seite'));
    write(originals.cve, cvePdf());
    write(originals.brief, docx());
    // Refused, each with its reason.
    write(path.join(pickedMedia.folder, 'Makro.docm'), docx(true));
    write(
      path.join(pickedMedia.folder, 'Programm.png'),
      Buffer.from('MZ\x90\x00\x03 not an image', 'latin1'),
    );
    write(path.join(pickedMedia.folder, 'Notiz.md'), '# Notiz\n');
    write(path.join(pickedMedia.folder, '.versteckt.png'), png(4, 4, [0, 0, 0]));
    write(pickedMedia.duplicate, fs.readFileSync(originals.ankunft));
    write(path.join(pickedMedia.cancelled, 'Eins.png'), png(30, 30, [200, 50, 50]));
    write(path.join(pickedMedia.cancelled, 'Zwei.png'), png(30, 30, [50, 200, 50]));
    originalsBefore = snapshot(mediaOriginals);
    titleBefore = await browser.getTitle();
  });

  it('imports a folder with subnets, names what it refuses and keeps the originals byte for byte', async () => {
    await runFromPalette('media.gallery');
    await $('[data-testid="gallery"]').waitForDisplayed();
    expect(await galleryTiles()).toHaveLength(0);
    await $('[data-testid="gallery-import-folder"]').click();
    const dialog = $('[data-testid="media-import"]');
    await dialog.waitForDisplayed({ timeout: 30_000 });
    const names: string[] = [];
    for (const item of await $$('[data-testid="media-import-items"] [data-import-item]').getElements()) {
      names.push(await item.getAttribute('data-import-item'));
    }
    expect(names.sort()).toEqual([
      'Ankunft.png',
      'Brief.docx',
      'Skript.svg',
      'Studie.pdf',
      'cve-2024-4367.pdf',
    ]);
    const reason = (name: string) => $(`[data-skipped="${name}"] [data-reason]`).getAttribute('data-reason');
    await $('[data-testid="media-import-skipped"] summary').click();
    expect(await reason('Makro.docm')).toBe('macro');
    expect(await reason('Programm.png')).toBe('type');
    expect(await reason('Notiz.md')).toBe('document');
    expect(await $('[data-skipped=".versteckt.png"]').isExisting()).toBe(false);
    expect(await $('[data-testid="media-import-from-folders"]').isSelected()).toBe(true);
    await $('[data-testid="media-import-confirm"]').click();
    await dialog.waitForExist({ reverse: true, timeout: 30_000 });
    await browser.waitUntil(async () => (await galleryTiles()).length === 5, { timeout: 30_000 });

    // Byte for byte: every original as before, every copy equal to its original.
    expect(snapshot(mediaOriginals)).toEqual(originalsBefore);
    const copies = mediaFiles();
    expect(copies).toHaveLength(5);
    const copySums = copies.map((file) => sha256(path.join(libraryRoot, 'media', file))).sort();
    const originalSums = Object.values(originals)
      .map((file) => sha256(file))
      .sort();
    expect(copySums).toEqual(originalSums);
    expect(importLeftovers()).toEqual([]);

    // The folders became subnets: Reise › Tag 1 › Abend.
    for (const name of ['Reise', 'Tag 1', 'Abend']) {
      const row = $(`//*[@data-subnet][.//span[normalize-space()="${name}"]]`);
      await row.waitForExist();
      if ((await row.getAttribute('aria-expanded')) === 'false') {
        await row.click();
        await browser.waitUntil(async () => (await row.getAttribute('aria-expanded')) === 'true');
      }
    }
    const subnetOf = async (title: string) =>
      $(
        `//*[@data-testid="explorer"]//*[@data-media][@data-subnet-member][.//span[normalize-space()="${title}"]]`,
      ).getAttribute('data-subnet-member');
    const subnetNamed = async (name: string) =>
      $(`//*[@data-subnet][.//span[normalize-space()="${name}"]]`).getAttribute('data-subnet');
    expect(await subnetOf('Ankunft')).toBe(await subnetNamed('Reise'));
    expect(await subnetOf('Studie')).toBe(await subnetNamed('Tag 1'));
    expect(await subnetOf('Brief')).toBe(await subnetNamed('Abend'));
  });

  it('shows an SVG with script as an image that runs nothing', async () => {
    await openTile('image', 'Skript');
    const image = $(`${SHOWN} [data-testid="media-image"]`);
    await browser.waitUntil(async () => Number(await image.getProperty('naturalWidth')) > 0, {
      timeoutMsg: 'the SVG was not shown',
    });
    // Time for an onload or a script to run if they could.
    await browser.pause(500);
    const after = await markers();
    expect(after.svg).toBeNull();
    expect(after.title).toBe(titleBefore);
    // The image is an image source, never part of the DOM.
    expect(await $('svg[onload]').isExisting()).toBe(false);
    expect(await image.getAttribute('src')).toMatch(/pwmedia/);
  });

  it('opens a PDF with pages, text and links, and the CVE-2024-4367 test PDF runs nothing', async () => {
    await runFromPalette('media.gallery');
    await openTile('pdf', 'Studie');
    const view = $(`${SHOWN} [data-testid="pdf-view"]`);
    await browser.waitUntil(async () => (await view.getAttribute('data-pages')) === '3', {
      timeout: 30_000,
      timeoutMsg: 'the PDF did not load',
    });
    await browser.waitUntil(
      async () => (await $(`${SHOWN} [data-testid="pdf-text"]`).getText()).includes('Studie Seite 1'),
      {
        timeoutMsg: 'no text layer',
      },
    );
    // An inner link jumps to its page; page buttons work.
    await $(`${SHOWN} [data-pdf-link="page:3"]`).click();
    await browser.waitUntil(async () => (await view.getAttribute('data-page')) === '3');
    await $(`${SHOWN} [data-testid="pdf-previous"]`).click();
    await browser.waitUntil(async () => (await view.getAttribute('data-page')) === '2');
    // A web link asks first; cancelling opens nothing.
    await $(`${SHOWN} [data-testid="pdf-page"]`).setValue('1');
    await browser.keys('Enter');
    await browser.waitUntil(async () => (await view.getAttribute('data-page')) === '1');
    await $(`${SHOWN} [data-pdf-link="https://example.org/quelle"]`).click();
    const prompt = $('[data-testid="pdf-link-prompt"]');
    await prompt.waitForDisplayed();
    await browser.keys('Escape');
    await prompt.waitForExist({ reverse: true });

    await runFromPalette('media.gallery');
    await openTile('pdf', 'cve-2024-4367');
    // Shown (its font refused or replaced) or refused as a whole: either way
    // nothing of the FontMatrix may run.
    await browser.waitUntil(
      () =>
        browser.execute(
          (shown) =>
            document.querySelector(`${shown} [data-testid="pdf-view"]`)?.getAttribute('data-pages') === '1' ||
            document.querySelector(`${shown} [data-testid="pdf-failed"]`) !== null,
          SHOWN,
        ),
      { timeout: 30_000, timeoutMsg: 'the CVE test PDF neither loaded nor failed' },
    );
    await browser.pause(1000);
    const after = await markers();
    expect(after.cve).toBeNull();
    expect(after.title).toBe(titleBefore);
  });

  it('offers the existing medium for a duplicate', async () => {
    await runFromPalette('media.gallery');
    await $('[data-testid="gallery-import"]').click();
    const dialog = $('[data-testid="media-import"]');
    await dialog.waitForDisplayed({ timeout: 30_000 });
    await $('[data-duplicate]').waitForDisplayed();
    expect(await $('[data-testid="media-import-duplicates"]').getValue()).toBe('useExisting');
    await $('[data-testid="media-import-confirm"]').click();
    await dialog.waitForExist({ reverse: true });
    await browser.pause(300);
    expect(await galleryTiles()).toHaveLength(5);
    expect(mediaFiles()).toHaveLength(5);
  });

  it('leaves neither files nor rows when an import is cancelled', async () => {
    const filesBefore = mediaFiles();
    await $('[data-testid="gallery-import-folder"]').click();
    const dialog = $('[data-testid="media-import"]');
    await dialog.waitForDisplayed({ timeout: 30_000 });
    expect(await $$('[data-testid="media-import-items"] [data-import-item]').getElements()).toHaveLength(2);
    // The copies wait in tmp/ while the dialog asks.
    expect(importLeftovers()).toHaveLength(1);
    await $('[data-testid="media-import-cancel"]').click();
    await dialog.waitForExist({ reverse: true });
    await browser.waitUntil(() => importLeftovers().length === 0, { timeoutMsg: 'copies left in tmp/' });
    expect(mediaFiles()).toEqual(filesBefore);
    expect(await galleryTiles()).toHaveLength(5);
    expect(snapshot(mediaOriginals)).toEqual(originalsBefore);
  });
});
