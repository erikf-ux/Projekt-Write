/**
 * Performance targets of section 6 for the writing surface (Phase 4):
 *
 * * "Dokument mit 50.000 Wörtern öffnen": ≤ 500 ms, from the click in the
 *   Explorer until the frame that shows the editable text has been drawn.
 * * "Tippverzögerung bei 50.000 Wörtern": 95 % of the key presses ≤ 50 ms,
 *   from the key press until the frame that shows the character has been
 *   drawn. (The language check comes with its own phase.)
 *
 * The keys are dispatched in the page the way the embedded WebDriver does it
 * (a `keydown`, then `document.execCommand('insertText')` unless a handler
 * cancelled it, ADR 0003), at the pace of a quick typist with pauses after
 * words and sentences, so autosave and statistics run in between as they
 * would. A WebDriver round trip per key would measure the driver instead.
 *
 * CI machines only show the trend (section 6), so the test fails only far
 * above the targets; the measured values are logged for the report.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import { END_MARKER, longGermanText } from '../longText';
import { pickedFolders, sandbox } from '../sandbox';
import { closeSettings, pinMainWindow } from '../session';

const TARGET_WORDS = 50_000;
const OPEN_TARGET_MS = 500;
const TYPING_TARGET_MS = 50;
/** Far above the targets: only a hang or a gross regression fails on CI. */
const OPEN_LIMIT_MS = 10 * OPEN_TARGET_MS;
const TYPING_LIMIT_MS = 10 * TYPING_TARGET_MS;

/**
 * 200 key presses: German text with umlauts, ß, punctuation and one Enter,
 * after a space behind the last full stop of a paragraph.
 */
const TYPED =
  ' Am Abend schrieb sie weiter, langsam und ohne Eile. Draußen fiel Regen auf die Straße, ' +
  'und über dem Tisch brannte die Lampe.\nSpäter öffnete sie das Fenster. Die Luft war kühl; ' +
  'nur ein Zug fuhr vorbei';

/** 100 key presses in the Markdown source, with one Enter. */
const SOURCE_TYPED =
  ' Nachts las sie den Text noch einmal, Satz für Satz, und strich, was zu laut war.\nDann schlief Anna.';

function documentFile(id: string): string {
  return path.join(pickedFolders.moveTarget, 'documents', 'markdown', `${id}.md`);
}

async function activeDocumentId(): Promise<string> {
  return (await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getAttribute('data-tab')) ?? '';
}

function percentile(values: number[], share: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(share * sorted.length) - 1)] ?? Number.NaN;
}

function summary(values: number[]): string {
  const fixed = (value: number) => value.toFixed(1);
  return `median ${fixed(percentile(values, 0.5))} ms, 95 % ${fixed(percentile(values, 0.95))} ms, max ${fixed(Math.max(...values))} ms`;
}

/** Closes the tab of a document with its close button. */
async function closeTab(id: string) {
  await $(`[data-testid="tabs"] [role="tab"][data-tab="${id}"] button`).click();
  await $(`[data-testid="tabs"] [role="tab"][data-tab="${id}"]`).waitForExist({ reverse: true });
}

/**
 * Clicks the document in the Explorer and measures until the frame with the
 * whole text, ready for input, has been drawn. Returns the view it opened in.
 */
async function measureOpen(id: string): Promise<{ ms: number; view: string }> {
  return browser.execute(
    async (documentId: string, marker: string) => {
      const option = document.querySelector<HTMLElement>(
        `[data-testid="explorer"] [data-document="${documentId}"]`,
      );
      if (!option) {
        throw new Error('the document is not in the Explorer');
      }
      const surface = () => {
        const panel = document.getElementById(`pw-panel-${documentId}`);
        if (!panel || panel.hidden) {
          return null;
        }
        for (const element of panel.querySelectorAll<HTMLElement>('[data-testid="editor"]')) {
          if (element.getClientRects().length > 0 && element.isContentEditable) {
            const last = [...element.children].slice(-3);
            if (last.some((child) => child.textContent.includes(marker))) {
              return element;
            }
          }
        }
        return null;
      };
      const start = performance.now();
      option.click();
      // Checked before each frame is drawn; a message posted then runs after it.
      const view = await new Promise<string>((resolve) => {
        const check = () => {
          const element = surface();
          if (element) {
            const channel = new MessageChannel();
            channel.port1.onmessage = () => {
              resolve(element.dataset['view'] ?? '');
            };
            channel.port2.postMessage(null);
          } else {
            requestAnimationFrame(check);
          }
        };
        requestAnimationFrame(check);
      });
      return { ms: performance.now() - start, view };
    },
    id,
    END_MARKER,
  );
}

/**
 * Types `text` at the end of the middle paragraph (rich text) or line
 * (source view), one key press after the other, and returns the latency of
 * each.
 */
async function measureTyping(id: string, text: string): Promise<number[]> {
  return browser.execute(
    async (documentId: string, keys: string) => {
      const panel = document.getElementById(`pw-panel-${documentId}`);
      const editor = [...(panel?.querySelectorAll<HTMLElement>('[data-testid="editor"]') ?? [])].find(
        (element) => element.getClientRects().length > 0,
      );
      if (!editor) {
        throw new Error('no visible writing surface');
      }
      const blocks = [...editor.querySelectorAll(':scope > p, :scope > [data-pw-line]')].filter(
        (block) => block.textContent.trim() !== '',
      );
      const target = blocks[Math.floor(blocks.length / 2)];
      if (!target) {
        throw new Error('no paragraph');
      }
      const frame = () =>
        new Promise<void>((resolve) => {
          requestAnimationFrame(() => {
            resolve();
          });
        });
      const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
      /** Resolves once the next frame has been drawn. */
      const drawn = () =>
        new Promise<void>((resolve) => {
          requestAnimationFrame(() => {
            const channel = new MessageChannel();
            channel.port1.onmessage = () => {
              resolve();
            };
            channel.port2.postMessage(null);
          });
        });
      target.scrollIntoView({ block: 'center' });
      editor.focus();
      const range = document.createRange();
      range.selectNodeContents(target);
      range.collapse(false);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      await frame();
      await frame();

      // A quick typist: about 9 keys a second, a little longer after a
      // word, about a second after a sentence (so autosave runs in between).
      let seed = 7;
      const next = () => {
        seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
        return seed / 2_147_483_648;
      };
      const latencies: number[] = [];
      for (const character of keys) {
        const key = character === '\n' ? 'Enter' : character;
        const options = { key, bubbles: true, cancelable: true, composed: true };
        const start = performance.now();
        const active = document.activeElement ?? editor;
        // The same calls as the embedded WebDriver's key actions (ADR 0003).
        if (active.dispatchEvent(new KeyboardEvent('keydown', options))) {
          // eslint-disable-next-line @typescript-eslint/no-deprecated -- the path the WebDriver types on
          document.execCommand(key === 'Enter' ? 'insertParagraph' : 'insertText', false, character);
        }
        active.dispatchEvent(new KeyboardEvent('keyup', options));
        await drawn();
        latencies.push(performance.now() - start);
        const pause = /[.!?;\n]/.test(character)
          ? 900 + next() * 300
          : character === ' '
            ? 120 + next() * 120
            : 70 + next() * 70;
        await sleep(Math.max(0, pause - (performance.now() - start)));
      }
      return latencies;
    },
    id,
    text,
  );
}

describe('Project Write – Phase 4 performance', () => {
  const text = longGermanText(TARGET_WORDS);
  let id = '';
  const results: string[] = [];

  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    await closeSettings();
    // Typing 200 keys at a human pace takes about half a minute.
    await browser.setTimeout({ script: 180_000 });
  });

  // For the report: logged, and kept in the sandbox until the next run.
  after(() => {
    const report = `Leistung (${process.platform}, ${String(text.words)} Wörter):\n${results.join('\n')}\n`;
    console.log(`\n${report}`);
    fs.writeFileSync(path.join(sandbox, 'performance.txt'), report);
  });

  it('imports a manuscript of 50,000 words and counts them right', async () => {
    expect(text.words).toBe(TARGET_WORDS);
    await browser.execute((content: string) => {
      const data = new DataTransfer();
      data.items.add(new File([content], 'Manuskript.md', { type: 'text/markdown' }));
      const target = document.querySelector('[data-testid="explorer"]') ?? document.body;
      for (const kind of ['dragenter', 'dragover', 'drop']) {
        target.dispatchEvent(new DragEvent(kind, { bubbles: true, cancelable: true, dataTransfer: data }));
      }
    }, text.markdown);
    await browser.waitUntil(
      async () =>
        (await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').getAttribute('title')) ===
        'Manuskript',
      { timeout: 30_000, timeoutMsg: 'the manuscript was not imported and opened' },
    );
    id = await activeDocumentId();
    expect(fs.readFileSync(documentFile(id), 'utf8')).toBe(text.markdown);
    await expect($('[data-testid="status-words"]')).toHaveText(expect.stringContaining('Wörter: 50.000'));
    await closeTab(id);
  });

  it('opens the document within the target', async () => {
    const runs: number[] = [];
    for (let run = 0; run < 3; run += 1) {
      const { ms, view } = await measureOpen(id);
      expect(view).toBe('rich');
      runs.push(ms);
      await closeTab(id);
    }
    results.push(
      `Öffnen (Ziel ≤ ${String(OPEN_TARGET_MS)} ms): ${runs.map((ms) => ms.toFixed(0)).join(', ')} ms`,
    );
    expect(Math.min(...runs)).toBeLessThan(OPEN_LIMIT_MS);
  });

  it('keeps the typing latency within the target', async () => {
    await measureOpen(id);
    const latencies = await measureTyping(id, TYPED);
    expect(latencies).toHaveLength(TYPED.length);
    results.push(
      `Tippen, ${String(latencies.length)} Anschläge (Ziel: 95 % ≤ ${String(TYPING_TARGET_MS)} ms): ${summary(latencies)}`,
    );
    // Everything typed arrived, in this order, and is saved.
    await browser.waitUntil(
      async () => (await $('[data-testid="status-save"]').getAttribute('data-state')) === 'saved',
      { timeout: 20_000, timeoutMsg: 'the typed text was not saved' },
    );
    const saved = fs.readFileSync(documentFile(id), 'utf8');
    for (const line of TYPED.split('\n')) {
      expect(saved).toContain(line);
    }
    expect(percentile(latencies, 0.95)).toBeLessThan(TYPING_LIMIT_MS);
  });

  it('keeps the typing latency within the target in the source view', async () => {
    await browser.keys(['Control', '/']);
    await browser.waitUntil(
      async () =>
        (await browser.execute(
          (documentId: string) =>
            [
              ...(document
                .getElementById(`pw-panel-${documentId}`)
                ?.querySelectorAll<HTMLElement>('[data-testid="editor"]') ?? []),
            ].find((element) => element.getClientRects().length > 0)?.dataset['view'] ?? '',
          id,
        )) === 'source',
      { timeoutMsg: 'Ctrl+/ did not switch to the source view' },
    );
    const latencies = await measureTyping(id, SOURCE_TYPED);
    expect(latencies).toHaveLength(SOURCE_TYPED.length);
    results.push(
      `Tippen in der Quelltextansicht, ${String(latencies.length)} Anschläge: ${summary(latencies)}`,
    );
    await browser.waitUntil(
      async () => (await $('[data-testid="status-save"]').getAttribute('data-state')) === 'saved',
      { timeout: 20_000, timeoutMsg: 'the typed text was not saved' },
    );
    const saved = fs.readFileSync(documentFile(id), 'utf8');
    for (const line of SOURCE_TYPED.split('\n')) {
      expect(saved).toContain(line);
    }
    expect(percentile(latencies, 0.95)).toBeLessThan(TYPING_LIMIT_MS);
  });

  it('opens the saved document within the target as well', async () => {
    await closeTab(id);
    const { ms, view } = await measureOpen(id);
    expect(view).toBe('rich');
    results.push(`Öffnen nach dem Speichern: ${ms.toFixed(0)} ms`);
    expect(ms).toBeLessThan(OPEN_LIMIT_MS);
  });
});
