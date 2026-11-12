/**
 * Export, printing and backups in the running app (Phase 10, section 21,
 * ADR 0021). The acceptance criteria of section 25 that the app can show:
 *
 * * printing and PDF go over print CSS and the system dialog (the dialog
 *   itself is a native window the WebDriver cannot operate, ADR 0003: here
 *   `window.print` is replaced and the print copy is checked);
 * * a backup while documents are being saved is consistent and checks out;
 * * a damaged backup is refused;
 * * restoring (stage 5) brings the library back in a new folder.
 *
 * Besides: TXT with the loss warning, Markdown, the package out and in
 * again, the library export. Hard kills while backing up, changed backups
 * and the exclusion of earlier backups are tested in Rust
 * (tests/backup.rs). Runs after crash-recovered.e2e.ts; the restore leaves
 * another library open, which reference-prepare.e2e.ts replaces anyway.
 * The dialogs answer from `PW_E2E_PICK_EXPORT` (sandbox.ts), in order.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import { activeDocumentId, appendText, renameActive, runFromPalette } from '../editing';
import { readPdf } from '../pdf';
import { backupsDir, exportsDir, pickedExport, printedPdf, restoreParent } from '../sandbox';
import { closeSettings, openSettings, pinMainWindow } from '../session';

async function notice(contains: string, tone: 'info' | 'error' = 'info'): Promise<string> {
  const bar = $(`[data-testid="notice"][data-tone="${tone}"]`);
  await browser.waitUntil(async () => (await bar.isExisting()) && (await bar.getText()).includes(contains), {
    timeout: 60_000,
    timeoutMsg: `no notice with "${contains}"`,
  });
  return bar.getText();
}

/** Invokes a command of the app directly (for the saving in the background). */
async function invoke<T>(command: string, request: Record<string, unknown>): Promise<T> {
  return browser.execute(
    async (name: string, body: Record<string, unknown>) => {
      const internals = (
        window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }
      ).__TAURI_INTERNALS__;
      return internals.invoke(name, { request: body });
    },
    command,
    request,
  ) as Promise<T>;
}

describe('Export, printing and backups (Phase 10)', () => {
  let report = '';

  before(async () => {
    await pinMainWindow();
    for (const dir of [exportsDir, backupsDir, restoreParent, path.dirname(printedPdf)]) {
      fs.mkdirSync(dir, { recursive: true });
    }
    await runFromPalette('document.newRichText');
    await browser.waitUntil(async () => (await activeDocumentId()) !== '');
    report = await activeDocumentId();
    await appendText('Hafen am Abend');
    await renameActive('Bericht');
  });

  it('exports Markdown, and TXT after the loss warning', async () => {
    await runFromPalette('export.markdown');
    await notice('Exportiert nach');
    await browser.waitUntil(() => fs.existsSync(pickedExport.markdown));
    expect(fs.readFileSync(pickedExport.markdown, 'utf8')).toContain('Hafen am Abend');

    // Rich text to TXT loses the formatting: asked first.
    await runFromPalette('export.txt');
    const prompt = $('[data-testid="export-loss-prompt"]');
    await prompt.waitForDisplayed({ timeoutMsg: 'no loss warning for TXT' });
    await prompt.$('button=Trotzdem exportieren').click();
    await browser.waitUntil(() => fs.existsSync(pickedExport.text));
    expect(fs.readFileSync(pickedExport.text, 'utf8').trim()).toBe('Hafen am Abend');
  });

  it('exports a package and imports it as a new document', async () => {
    await runFromPalette('export.pack');
    await notice('Paket gespeichert');
    expect(fs.existsSync(pickedExport.pack)).toBe(true);
    await runFromPalette('import.pack');
    await notice('Paket importiert');
    await browser.waitUntil(async () => {
      const id = await activeDocumentId();
      return id !== '' && id !== report;
    });
    const title = await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getText();
    expect(title).toContain('Bericht');
  });

  it('exports the library as a folder with an index', async () => {
    await runFromPalette('export.library');
    await notice('Bibliothek exportiert');
    const folder = fs.readdirSync(exportsDir).find((name) => name.startsWith('Project Write Export'));
    expect(folder).toBeDefined();
    const index = fs.readFileSync(path.join(exportsDir, folder ?? '', 'index.md'), 'utf8');
    expect(index).toContain('Bericht');
    // Nothing half-written stays next to it.
    expect(fs.readdirSync(exportsDir).filter((name) => name.includes('.partial-'))).toEqual([]);
  });

  it('prints only a copy of the writing surface, over print CSS', async () => {
    await $(`[data-testid="tabs"] [data-tab="${report}"]`).click();
    await browser.waitUntil(async () => (await activeDocumentId()) === report);
    await browser.execute(() => {
      const record = window as unknown as { pwPrinted: { title: string; text: string; printing: boolean }[] };
      record.pwPrinted = [];
      window.print = () => {
        record.pwPrinted.push({
          title: document.title,
          text: document.getElementById('pw-print')?.textContent ?? '',
          printing: document.documentElement.hasAttribute('data-printing'),
        });
      };
    });
    await browser.keys(['Control', 'p']);
    await browser.waitUntil(() =>
      browser.execute(() => (window as unknown as { pwPrinted: unknown[] }).pwPrinted.length === 1),
    );
    const printed = await browser.execute(
      () =>
        (window as unknown as { pwPrinted: { title: string; text: string; printing: boolean }[] })
          .pwPrinted[0],
    );
    expect(printed?.title).toBe('Bericht');
    expect(printed?.text).toContain('Hafen am Abend');
    expect(printed?.printing).toBe(true);
    // The print rules are there: in print only #pw-print shows.
    const rules = await browser.execute(() =>
      [...document.styleSheets]
        .flatMap((sheet) => {
          try {
            return [...sheet.cssRules];
          } catch {
            return [];
          }
        })
        .filter((rule) => rule instanceof CSSMediaRule && rule.conditionText.includes('print'))
        .map((rule) => rule.cssText)
        .join('\n'),
    );
    expect(rules).toContain('#pw-print');
    // Closing the dialog removes the copy.
    await browser.execute(() => {
      window.dispatchEvent(new Event('afterprint'));
    });
    expect(await $('#pw-print').isExisting()).toBe(false);

    await runFromPalette('export.pdf');
    await notice('Als PDF speichern');
    await browser.execute(() => {
      window.dispatchEvent(new Event('afterprint'));
    });
  });

  it('prints a document of several pages with image and link to a PDF in the engine of the webview', async () => {
    // A medium of media.e2e.ts, a raster image.
    const media = await invoke<{ id: string; kind: string; mimeType: string }[]>('media_list', {});
    const image = media.find((medium) => medium.kind === 'image' && medium.mimeType === 'image/png');
    expect(image).toBeDefined();
    const paragraph = (n: number) =>
      `Absatz ${String(n)}: Die Fähre legt am Kai an, die Möwen kreisen über dem Wasser, und im Kontor zählt jemand die Kisten ein, eine nach der anderen, bis der Abend kommt und das Licht über dem Hafen langsam verschwindet.`;
    const markdown = [
      '# Hafenchronik',
      '',
      'Ein **fetter** Satz und ein Link zum [Hafenamt](https://example.org/hafenamt).',
      '',
      `![Ankunft](pwmedia://${image?.id ?? ''})`,
      '',
      '- Erster Punkt',
      '- Zweiter Punkt',
      '',
      ...Array.from({ length: 40 }, (_, n) => [paragraph(n + 1), '']).flat(),
      'Schlusswort Leuchtfeuer.',
      '',
    ].join('\n');
    const created = await invoke<{ id: string }>('document_create', { title: 'Druckprobe', format: 'md' });
    await invoke('document_save', { id: created.id, text: markdown });

    // Opened over the search, the way a user finds it.
    await browser.keys(['Control', 'Shift', 'f']);
    await $('[data-testid="search-field"]').waitForDisplayed();
    await browser.execute(() => {
      const field = document.querySelector<HTMLInputElement>('[data-testid="search-field"]');
      field?.focus();
      field?.select();
      // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
      document.execCommand('insertText', false, 'Leuchtfeuer');
    });
    const hit = $(`[data-testid="search-results"] [data-hit="${created.id}"]`);
    await hit.waitForDisplayed({ timeoutMsg: 'the test document was not found' });
    await hit.click();
    await browser.waitUntil(async () => (await activeDocumentId()) === created.id);
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const picture = document.querySelector<HTMLImageElement>(
            '[role="tabpanel"]:not([hidden]) [data-testid="editor"] [data-pw-media-image] img',
          );
          return picture !== null && picture.complete && picture.naturalWidth > 0;
        }),
      { timeoutMsg: 'the image was not shown in the text' },
    );

    // The print copy as Ctrl+P makes it; the engine prints the page
    // (WebKitGTK: a print operation to a file, WebView2: PrintToPdf, the way
    // "Als PDF speichern" does). The dialog itself is the system's.
    await browser.execute(() => {
      window.print = () => undefined;
    });
    await browser.keys(['Control', 'p']);
    await $('#pw-print').waitForExist({ timeoutMsg: 'Ctrl+P made no print copy' });
    // Opened from a search hit, the find bar marks the hit on screen; the
    // print copy has no such marks (F-52).
    expect(
      await browser.execute(() => ({
        screen: document.querySelectorAll('[data-testid="editor"] .find-and-replace-result').length,
        print: document.querySelectorAll('#pw-print .find-and-replace-result').length,
      })),
    ).toEqual({ screen: 1, print: 0 });
    const pdf = Buffer.from(await browser.printPage(), 'base64');
    await browser.execute(() => {
      window.dispatchEvent(new Event('afterprint'));
    });
    fs.writeFileSync(printedPdf, pdf);

    const pages = await readPdf(new Uint8Array(pdf));
    const all = pages.map((page) => page.text).join(' ');
    console.log(
      `Printed PDF (${process.platform}): ${String(pages.length)} pages of ${pages[0]?.width.toFixed(0) ?? '?'} x ${pages[0]?.height.toFixed(0) ?? '?'} pt, ${String(pdf.length)} bytes; images ${pages.map((page) => page.images).join('/')}; links ${pages.flatMap((page) => page.links).join(' ')}`,
    );
    // Several pages, in order, with nothing cut off.
    expect(pages.length).toBeGreaterThanOrEqual(3);
    expect(pages[0]?.text).toContain('Druckprobe');
    expect(pages[0]?.text).toContain('Hafenchronik');
    expect(pages.at(-1)?.text).toContain('Leuchtfeuer');
    for (let n = 1; n <= 40; n += 1) {
      expect(all).toContain(`Absatz ${String(n)}:`);
    }
    // Only the document: no tab, no Explorer, no other window content.
    expect(all).not.toContain('Bericht');
    expect(all).not.toContain('Papierkorb');
    // The image is printed, the link shows its address. WebView2 keeps the
    // link as a link area; WebKitGTK prints no link areas (bericht-phase-10.md).
    expect(pages.reduce((sum, page) => sum + page.images, 0)).toBeGreaterThanOrEqual(1);
    expect(all).toContain('example.org/hafenamt');
    if (process.platform === 'win32') {
      expect(pages.flatMap((page) => page.links)).toContain('https://example.org/hafenamt');
    }
    // Text stays inside the page, with a margin on every side.
    for (const page of pages) {
      const box = page.textBox;
      expect(box).not.toBeNull();
      expect(box?.left ?? 0).toBeGreaterThanOrEqual(18);
      expect(box?.right ?? Infinity).toBeLessThanOrEqual(page.width - 18);
      expect(box?.bottom ?? 0).toBeGreaterThanOrEqual(18);
      expect(box?.top ?? Infinity).toBeLessThanOrEqual(page.height - 18);
    }
    // Back to the screen as before.
    expect(await $('#pw-print').isExisting()).toBe(false);
    await $(`[data-testid="tabs"] [data-tab="${report}"]`).click();
  });

  it('makes a consistent backup while a document is being saved, checks it and refuses a damaged one', async () => {
    await openSettings('library');
    await $('[data-testid="backup-preferences"]').waitForDisplayed();
    await $('[data-testid="backup-choose-target"]').click();
    await expect($('[data-testid="backup-target"]')).toHaveText(expect.stringContaining('Sicherungen'));

    // Saving in the background while the backup runs.
    const created = await invoke<{ id: string }>('document_create', { title: 'Laufend', format: 'md' });
    await browser.execute((id: string) => {
      const internals = (
        window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }
      ).__TAURI_INTERNALS__;
      const state = window as unknown as { pwSaving: boolean; pwSaved: number };
      state.pwSaving = true;
      state.pwSaved = 0;
      void (async () => {
        while (state.pwSaving) {
          state.pwSaved += 1;
          await internals.invoke('document_save', {
            request: { id, text: `Fassung ${String(state.pwSaved)}\n`.repeat(200) },
          });
        }
      })();
    }, created.id);
    await $('[data-testid="backup-create"]').click();
    const done = await notice('Backup erstellt und geprüft');
    const saved = await browser.execute(() => {
      const state = window as unknown as { pwSaving: boolean; pwSaved: number };
      state.pwSaving = false;
      return state.pwSaved;
    });
    expect(saved).toBeGreaterThan(1);
    expect(done).toMatch(/\d+ Dokumente/);
    await expect($('[data-testid="backup-last"]')).toHaveText(expect.stringContaining('Letztes Backup'));

    // The backup in the target looks finished and is the only file there.
    const backups = fs.readdirSync(backupsDir).filter((name) => name.endsWith('.pwbackup'));
    expect(backups).toHaveLength(1);
    expect(fs.readdirSync(backupsDir).filter((name) => name.endsWith('.part'))).toEqual([]);
    fs.copyFileSync(path.join(backupsDir, backups[0] ?? ''), pickedExport.check);
    // A damaged copy: cut in half.
    const bytes = fs.readFileSync(pickedExport.check);
    fs.writeFileSync(pickedExport.damaged, bytes.subarray(0, Math.floor(bytes.length / 2)));

    await $('[data-testid="backup-check"]').click();
    await notice('Backup in Ordnung');
    await $('[data-testid="backup-check"]').click();
    await notice('nicht in Ordnung', 'error');
    report = created.id;
  });

  it('restores the backup into a new folder (stage 5) and opens it', async () => {
    await $('[data-testid="backup-restore"]').click();
    await notice('wiederhergestellt');
    const restored = fs
      .readdirSync(restoreParent)
      .filter((name) => name.startsWith('ProjectWrite-Wiederherstellung-'));
    expect(restored).toHaveLength(1);
    const status = await invoke<{ state: string; library: { displayPath: string } }>('library_status', {});
    expect(status.state).toBe('open');
    expect(status.library.displayPath).toContain('Wiederherstellung');
    // The document saved while the backup ran is there, as one whole version.
    const read = await invoke<{ text: string }>('document_read', { id: report });
    expect(read.text).toMatch(/^(Fassung \d+\n)+$/);
    const lines = new Set(read.text.trim().split('\n'));
    expect(lines.size).toBe(1);
    await closeSettings();
  });
});
