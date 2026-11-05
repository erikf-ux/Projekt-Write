/**
 * Subnets and tags in the running app (Phase 7, ADR 0017). The acceptance
 * criteria of section 25: cycles are refused; a document in two subnets
 * stays one record and one file; deleting a subnet deletes no element.
 * Besides: colours, breadcrumbs, tags, the filter, moving and the
 * structural undo.
 *
 * Runs after documents.e2e.ts in the same library.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { libraryRoot, newDocument, renameActive, runFromPalette } from '../editing';
import { closeSettings, pinMainWindow } from '../session';

const markdownFolder = path.join(libraryRoot, 'documents', 'markdown');

function subnetRow(name: string) {
  return $(`//*[@data-subnet][.//span[normalize-space()="${name}"]]`);
}

async function subnetId(name: string): Promise<string> {
  return (await subnetRow(name).getAttribute('data-subnet')) ?? '';
}

/** Opens a subnet in the Explorer if it is closed. */
async function expand(name: string) {
  const row = subnetRow(name);
  if ((await row.getAttribute('aria-expanded')) === 'false') {
    await row.click();
  }
  await browser.waitUntil(async () => (await subnetRow(name).getAttribute('aria-expanded')) === 'true');
}

async function createSubnet(name: string, color: string) {
  const dialog = $('[data-testid="subnet-dialog"]');
  await dialog.waitForDisplayed();
  await $('[data-testid="subnet-name"]').setValue(name);
  await $(`[data-testid="subnet-dialog"] [data-color-choice="${color}"]`).click();
  await $('[data-testid="subnet-save"]').click();
  await dialog.waitForExist({ reverse: true });
  await subnetRow(name).waitForExist({ timeoutMsg: `subnet ${name} is not in the Explorer` });
}

/** The Explorer rows of a document. */
async function rowsOf(id: string): Promise<string[]> {
  const places: string[] = [];
  for (const row of await $$(`[data-testid="explorer"] [data-document="${id}"]`).getElements()) {
    places.push(
      (await row.getAttribute('data-subnet-member')) ?? (await row.getAttribute('data-group-member')) ?? '',
    );
  }
  return places.sort();
}

/** Drags one Explorer row onto another (synthetic HTML5 drag and drop, ADR 0003). */
async function drag(from: string, to: string) {
  await browser.execute(
    (source: string, target: string) => {
      const start = document.querySelector<HTMLElement>(source);
      const end = document.querySelector<HTMLElement>(target);
      if (!start || !end) {
        throw new Error('no row to drag or drop on');
      }
      const data = new DataTransfer();
      start.dispatchEvent(
        new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: data }),
      );
      end.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer: data }));
      end.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: data }));
    },
    from,
    to,
  );
}

async function pick(command: string, search: string) {
  await runFromPalette(command);
  const field = $('[data-testid="subnet-picker-search"]');
  await field.waitForDisplayed();
  await field.setValue(search);
  await browser.keys('Enter');
  await $('[data-testid="subnet-picker"]').waitForExist({ reverse: true });
}

describe('Subnets and tags (Phase 7)', () => {
  let scene = '';

  before(async () => {
    await pinMainWindow();
    await closeSettings();
  });

  it('builds a hierarchy with colours and refuses cycles', async () => {
    await $('[data-testid="explorer-new-subnet"]').click();
    await createSubnet('Projekte', 'green');
    await expect($(`[data-subnet="${await subnetId('Projekte')}"] [data-color]`)).toHaveAttribute(
      'data-color',
      'green',
    );
    // A subnet inside, from the context menu (Shift+F10 on the selected row).
    await subnetRow('Projekte').click();
    await browser.keys(['Shift', 'F10']);
    await $('[data-testid="context-menu"] [data-item="newChild"]').click();
    await createSubnet('Roman', 'violet');
    await expand('Projekte');
    await expect(subnetRow('Roman')).toHaveAttribute('aria-level', '2');

    // Projekte into Roman (its own subnet): refused, the tree stays.
    await drag(`[data-subnet="${await subnetId('Projekte')}"]`, `[data-subnet="${await subnetId('Roman')}"]`);
    const notice = $('[data-testid="notice"][data-tone="error"]');
    await notice.waitForDisplayed({ timeoutMsg: 'the cycle was not refused visibly' });
    expect(await notice.getText()).toContain('nicht in sich selbst');
    await expect(subnetRow('Projekte')).toHaveAttribute('aria-level', '1');
    await expect(subnetRow('Roman')).toHaveAttribute('aria-level', '2');
    await $('[data-testid="notice"] button[aria-label]').click();
  });

  it('keeps a document in two subnets as one record and one file', async () => {
    const filesBefore = fs.readdirSync(markdownFolder).length;
    scene = await newDocument();
    await renameActive('Szene');
    expect(fs.readdirSync(markdownFolder).length).toBe(filesBefore + 1);

    await pick('document.moveTo', 'Projekte');
    await pick('document.assignTo', 'Roman');
    await expand('Projekte');
    await expand('Roman');
    const projects = await subnetId('Projekte');
    const novel = await subnetId('Roman');
    await browser.waitUntil(async () => (await rowsOf(scene)).length === 2, {
      timeoutMsg: 'the document does not appear in both subnets',
    });
    expect(await rowsOf(scene)).toEqual([novel, projects].sort());
    // One element: the further membership carries the mark, the file is one.
    await expect($(`[data-subnet-member="${novel}"][data-document="${scene}"]`)).toHaveAttribute(
      'data-secondary',
      'true',
    );
    expect(fs.readdirSync(markdownFolder).filter((name) => name.startsWith(scene))).toEqual([`${scene}.md`]);
    expect(fs.readdirSync(markdownFolder).length).toBe(filesBefore + 1);

    // Breadcrumbs: the primary path, the other subnet.
    const crumbs = $('[data-testid="breadcrumbs"]');
    await expect(crumbs).toHaveText(expect.stringContaining('Projekte'));
    await expect($('[data-testid="breadcrumbs-others"]')).toHaveText(expect.stringContaining('Roman'));
  });

  it('gives tags and filters by them', async () => {
    await runFromPalette('document.tags');
    await $('[data-testid="tags-input"]').setValue('Entwurf');
    await $('[data-testid="tags-submit"]').click();
    await expect($('[data-testid="tags-current"]')).toHaveText(expect.stringContaining('Entwurf'));
    await browser.keys('Escape');
    await $('[data-testid="tags-dialog"]').waitForExist({ reverse: true });
    await expect($('[data-testid="breadcrumbs-tags"]')).toHaveText(expect.stringContaining('#Entwurf'));

    await $('[data-testid="explorer-filter-toggle"]').click();
    await $('[data-testid="explorer-filter-tags"]').click();
    await $('//*[@data-tag-choice][contains(., "Entwurf")]').click();
    await browser.waitUntil(
      async () => {
        const ids = new Set<string>();
        for (const row of await $$('[data-testid="explorer"] [data-document]').getElements()) {
          ids.add((await row.getAttribute('data-document')) ?? '');
        }
        return ids.size === 1 && ids.has(scene);
      },
      { timeoutMsg: 'the filter shows other documents' },
    );
    await $('[data-testid="explorer-filter-toggle"]').click();
    await $('[data-testid="explorer-filter"]').waitForExist({ reverse: true });
  });

  it('deletes a subnet without deleting an element, and undoes it', async () => {
    const projects = await subnetId('Projekte');
    const novel = await subnetId('Roman');
    await subnetRow('Projekte').click();
    // The tree has the focus (and keeps the selected row).
    await browser.execute(() => {
      document.getElementById('pw-explorer-list')?.focus();
    });
    await browser.waitUntil(async () =>
      browser.execute(() => {
        const tree = document.getElementById('pw-explorer-list');
        const row = document.getElementById(tree?.getAttribute('aria-activedescendant') ?? '');
        return document.activeElement === tree && row?.hasAttribute('data-subnet') === true;
      }),
    );
    await browser.keys('Delete');
    const prompt = $('[data-testid="subnet-delete-prompt"]');
    await prompt.waitForDisplayed();
    expect(await prompt.getText()).toContain('Kein Dokument wird gelöscht');
    await $('[data-testid="subnet-delete-prompt"] [data-choice="liftUp"]').click();
    await subnetRow('Projekte').waitForExist({ reverse: true });
    // Roman moved up; the document stays, in Roman.
    await expect(subnetRow('Roman')).toHaveAttribute('aria-level', '1');
    expect(fs.existsSync(path.join(markdownFolder, `${scene}.md`))).toBe(true);
    await expand('Roman');
    expect(await rowsOf(scene)).toEqual([novel]);

    // Ctrl+Z in the Explorer brings subnet and membership back.
    await browser.execute(() => {
      document.getElementById('pw-explorer-list')?.focus();
    });
    await browser.keys(['Control', 'z']);
    await subnetRow('Projekte').waitForExist({ timeoutMsg: 'undo did not bring the subnet back' });
    await expect($('[data-testid="notice"]')).toHaveText(expect.stringContaining('Rückgängig gemacht'));
    expect(await subnetId('Projekte')).toBe(projects);
    await expand('Projekte');
    await expect(subnetRow('Roman')).toHaveAttribute('aria-level', '2');
    await expand('Roman');
    await browser.waitUntil(async () => (await rowsOf(scene)).length === 2);
    expect(await rowsOf(scene)).toEqual([novel, projects].sort());
  });
});
