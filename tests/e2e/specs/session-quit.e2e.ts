/**
 * Stage 1, "Sitzung" (ADR 0016, decision 10), first half: when the window
 * closes, the open tabs, the active tab and the selection of each tab are
 * saved for the next start. session-restored.e2e.ts checks them after the
 * restart.
 *
 * Runs after documents.e2e.ts; ends the app (the test service starts it
 * again for the next file).
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import {
  activeDocumentId,
  activeEditor,
  documentFile,
  newDocument,
  renameActive,
  typeText,
} from '../editing';
import { appDirs, handover } from '../sandbox';
import { closeSettings, pinMainWindow, waitUntilAppEnded } from '../session';

interface SavedTab {
  kind: 'library' | 'external';
  id?: string;
  path?: string;
  anchor: number;
  head: number;
  view: string;
}

async function activate(id: string) {
  await $(`[data-testid="tabs"] [data-tab="${id}"]`).click();
  await browser.waitUntil(async () => (await activeDocumentId()) === id);
}

describe('Session at quit (stage 1)', () => {
  before(async () => {
    await pinMainWindow();
    await closeSettings();
  });

  it('saves the tabs, the active tab and the selection when the window closes', async () => {
    const first = await newDocument();
    await renameActive('Sitzung eins');
    await (await activeEditor()).click();
    await typeText('Anfang Mitte Ende');
    await browser.waitUntil(
      () =>
        fs.existsSync(documentFile(first, 'md')) &&
        fs.readFileSync(documentFile(first, 'md'), 'utf8') === 'Anfang Mitte Ende\n',
      { timeout: 10_000, timeoutMsg: 'the text was not saved' },
    );
    const second = await newDocument();
    await renameActive('Sitzung zwei');

    // "Mitte" selected in the first document: ProseMirror positions 8 to 13.
    await activate(first);
    await (await activeEditor()).click();
    await browser.execute(() => {
      const paragraph = document.querySelector('[role="tabpanel"]:not([hidden]) [data-testid="editor"] p');
      const text = paragraph?.firstChild;
      if (!text || text.nodeType !== Node.TEXT_NODE) {
        throw new Error('no text in the paragraph');
      }
      const range = document.createRange();
      range.setStart(text, 'Anfang '.length);
      range.setEnd(text, 'Anfang Mitte'.length);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    });
    await expect($('[data-testid="status-words"]')).toHaveText('Wörter: 1 von 3');

    // The active tab changes right before closing (saving waits 2 s after
    // a change): only the save at quit knows it.
    await activate(second);
    // WebdriverIO ends the session when the last window goes away and
    // reports that as an error; here it is the expected outcome.
    await browser.closeWindow().catch((error: unknown) => {
      if (!String(error).includes('All window handles were removed')) {
        throw error;
      }
    });
    await waitUntilAppEnded();

    const settings = JSON.parse(fs.readFileSync(path.join(appDirs, 'config', 'settings.json'), 'utf8')) as {
      session?: { tabs: SavedTab[]; active: number | null };
    };
    const session = settings.session;
    expect(session).toBeDefined();
    const tabs = session?.tabs ?? [];
    expect(tabs.find((tab) => tab.id === first)).toEqual({
      kind: 'library',
      id: first,
      anchor: 8,
      head: 13,
      view: 'rich',
    });
    expect(tabs[session?.active ?? -1]?.id).toBe(second);
    fs.writeFileSync(
      handover,
      JSON.stringify({
        first,
        second,
        library: tabs.filter((tab) => tab.kind === 'library').map((tab) => tab.id),
        external: tabs.filter((tab) => tab.kind === 'external').length,
      }),
    );
  });
});
