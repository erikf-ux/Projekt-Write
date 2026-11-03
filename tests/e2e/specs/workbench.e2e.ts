/**
 * Phase 3 in the running app (section 25): main layout, keyboard access
 * with visible focus, command palette, shortcut conflicts, drag and drop,
 * focus mode, zoom, themes, side bar.
 *
 * Runs after restart.e2e.ts: the library lies in the sync folder of the
 * sandbox (rollback mode) and holds the document of the first start.
 *
 * The embedded WebDriver sends synthetic events (ADR 0003): key presses
 * reach the page's listeners, and drag and drop is dispatched as HTML5
 * events with real `File` objects – the same path a drop from the system
 * takes with `dragDropEnabled: false`.
 */
import fs from 'node:fs';
import path from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { themes, type ThemeName } from '../../../src/modules/themes/tokens';
import { newAccentPixels, settledFrame } from '../pixels';
import { appDirs, pickedFolders } from '../sandbox';
import { closeSettings, openSettings, pinMainWindow } from '../session';

const library = pickedFolders.moveTarget;

function readSettings(): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(path.join(appDirs, 'config', 'settings.json'), 'utf8')) as Record<
    string,
    unknown
  >;
}

/** Types into the active writing surface, key by key. */
async function type(text: string) {
  const editor = await activeEditor();
  await editor.waitForDisplayed();
  await editor.click();
  await browser.keys(text.split(''));
}

async function activeEditor() {
  const panels = await $$('[role="tabpanel"]').getElements();
  for (const panel of panels) {
    if (!(await panel.getAttribute('hidden'))) {
      return panel.$('textarea');
    }
  }
  throw new Error('no visible writing surface');
}

async function tabTitles(): Promise<string[]> {
  const tabs = await $$('[data-testid="tabs"] [role="tab"]').getElements();
  const titles: string[] = [];
  for (const tab of tabs) {
    titles.push((await tab.getAttribute('title')) ?? '');
  }
  return titles;
}

async function saveState(): Promise<string | null> {
  return $('[data-testid="status-save"]').getAttribute('data-state');
}

/** The Markdown file of a document in the library. */
function documentFile(id: string): string {
  return path.join(library, 'documents', 'markdown', `${id}.md`);
}

async function activeDocumentId(): Promise<string> {
  const tab = $('[data-testid="tabs"] [role="tab"][aria-selected="true"]');
  return (await tab.getAttribute('data-tab')) ?? '';
}

describe('Project Write – Phase 3 workbench', () => {
  before(async () => {
    await pinMainWindow();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    await closeSettings();
  });

  it('shows the main layout of section 11 with the functions of this phase only', async () => {
    await expect($('[data-testid="toolbar"]')).toBeDisplayed();
    await expect($('[data-testid="activity-bar"]')).toBeDisplayed();
    await expect($('[data-testid="explorer"]')).toBeDisplayed();
    await expect($('[data-testid="status-bar"]')).toBeDisplayed();
    await expect($('[data-testid="status-rollback"]')).toBeDisplayed();
    // Explorer and writing surface; search and Mesh come with their phases.
    const views = await $$('[data-testid="activity-bar"] button').getElements();
    const commands: string[] = [];
    for (const view of views) {
      commands.push((await view.getAttribute('data-command')) ?? '');
    }
    expect(commands).toEqual(['explorer.focus', 'editor.focus', 'settings.open']);
    // The document of the first start is in the Explorer.
    await expect($('[data-testid="explorer"] [role="option"]')).toHaveText(
      expect.stringContaining('Unbenannt'),
    );
  });

  it('creates a document with Ctrl+N and saves the text 1 s after typing', async () => {
    await $('[data-testid="explorer"] [role="listbox"]').click();
    await browser.keys(['Control', 'n']);
    await browser.waitUntil(async () => (await tabTitles()).includes('Unbenannt 2'), {
      timeoutMsg: 'Ctrl+N did not open a new document',
    });
    const id = await activeDocumentId();
    await type('Hallo Welt');
    await browser.waitUntil(async () => (await saveState()) === 'saved', {
      timeout: 10_000,
      timeoutMsg: 'the text was not saved',
    });
    await expect($('[data-testid="status-save"]')).toHaveText('Gespeichert');
    expect(fs.readFileSync(documentFile(id), 'utf8')).toBe('Hallo Welt');
  });

  it('shows only working commands in the command palette and runs them', async () => {
    await browser.keys(['Control', 'Shift', 'p']);
    const palette = $('[data-testid="command-palette"]');
    await palette.waitForDisplayed();
    const options = await palette.$$('[role="option"]').getElements();
    const ids: string[] = [];
    for (const option of options) {
      ids.push((await option.getAttribute('data-command')) ?? '');
    }
    expect(ids).toContain('document.new');
    expect(ids).toContain('focus.toggle');
    // Everything is saved and the zoom is 100 %: these would do nothing.
    expect(ids).not.toContain('document.save');
    expect(ids).not.toContain('zoom.reset');
    // The palette does not list itself.
    expect(ids).not.toContain('palette.show');

    await browser.keys('Fokus'.split(''));
    await browser.keys('Enter');
    await expect(palette).not.toBeExisting();
    await expect($('[data-testid="workbench"]')).toHaveAttribute('data-focus-mode', 'true');
    await expect($('[data-testid="toolbar"]')).not.toBeExisting();
    await expect($('[data-testid="status-bar"]')).not.toBeExisting();
    await expect(await activeEditor()).toBeDisplayed();
    await browser.keys('Escape');
    await expect($('[data-testid="workbench"]')).toHaveAttribute('data-focus-mode', 'false');
    await expect($('[data-testid="toolbar"]')).toBeDisplayed();
  });

  it('reports a shortcut that is already taken before assigning it', async () => {
    await openSettings('shortcuts');
    await $('[data-testid="shortcut-change-palette.show"]').click();
    await $('[data-testid="shortcut-recorder"]').waitForDisplayed();
    await browser.keys(['Control', 'n']);
    const question = $('[data-testid="shortcut-conflict-question"]');
    await expect(question).toHaveText(expect.stringContaining('Neues Dokument'));
    await question.$('button=Abbrechen').click();
    await expect(question).not.toBeExisting();
    await closeSettings();
  });

  it('reports conflicts in the settings file and runs nothing on such a chord', async () => {
    // As if settings.json had been edited by hand: two commands on Ctrl+N.
    await browser.execute(async () => {
      const internals = (
        window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }
      ).__TAURI_INTERNALS__;
      const state = (await internals.invoke('ui_state', { request: {} })) as {
        settings: Record<string, unknown>;
      };
      await internals.invoke('ui_update_settings', {
        request: { settings: { ...state.settings, keybindings: { 'palette.show': ['Ctrl+N'] } } },
      });
    });
    // The interface reads the settings at start.
    await browser.refresh();
    await $('[data-testid="workbench"]').waitForDisplayed({ timeout: 60_000 });
    await expect($('[data-testid="status-conflicts"]')).toHaveText(expect.stringContaining('1'));
    const tabsBefore = (await tabTitles()).length;
    await $('[data-testid="explorer"] [role="listbox"]').click();
    await browser.keys(['Control', 'n']);
    await expect($('[data-testid="notice"]')).toHaveText(expect.stringContaining('mehreren Befehlen'));
    expect((await tabTitles()).length).toBe(tabsBefore);
    await expect($('[data-testid="command-palette"]')).not.toBeExisting();

    await $('[data-testid="status-conflicts"]').click();
    await expect($('[data-testid="shortcut-conflicts"]')).toHaveText(
      expect.stringContaining('Befehlspalette'),
    );
    await $('[data-shortcut-row="palette.show"]').$('button=Standard').click();
    await expect($('[data-testid="shortcut-conflicts"]')).not.toBeExisting();
    expect(readSettings()['keybindings']).toEqual({});
    await closeSettings();
  });

  it('reaches every region with the keyboard and shows the focus', async () => {
    // Composite widgets (toolbar, tabs) have one tab stop and arrow keys;
    // every other control is a tab stop of its own.
    const structure = await browser.execute(() => {
      const composites = ['[role="toolbar"]', '[role="tablist"]'];
      const buttons = [...document.querySelectorAll<HTMLButtonElement>('[data-testid="workbench"] button')];
      const unreachable = buttons
        .filter((button) => !button.disabled && button.tabIndex < 0)
        .filter((button) => !composites.some((selector) => button.closest(selector)))
        .map((button) => button.outerHTML.slice(0, 80));
      const stops = composites.flatMap((selector) =>
        [...document.querySelectorAll<HTMLElement>(selector)].map(
          (element) => element.querySelectorAll('[tabindex="0"]').length,
        ),
      );
      return { unreachable, stops };
    });
    expect(structure.unreachable).toEqual([]);
    expect(structure.stops.every((count) => count === 1)).toBe(true);

    const theme = (await browser.execute(
      () => document.documentElement.dataset['theme'] ?? 'light',
    )) as ThemeName;
    const accent = themes[theme in themes ? theme : 'light'].accent;
    const regions = [
      '[data-testid="toolbar"] [tabindex="0"]',
      '[data-testid="activity-bar"] [tabindex="0"]',
      '#pw-explorer-list',
      '[data-testid="sidebar-resize"]',
      '[data-testid="tabs"] [role="tab"][tabindex="0"]',
      '[role="tabpanel"]:not([hidden]) textarea',
    ];
    const problems: string[] = [];
    for (const selector of regions) {
      await browser.execute(() => {
        (document.activeElement as HTMLElement | null)?.blur();
      });
      const { image: before } = await settledFrame();
      const rect = await browser.execute((target: string) => {
        const element = document.querySelector<HTMLElement>(target);
        if (!element) {
          return null;
        }
        element.focus({ focusVisible: true });
        const box = element.getBoundingClientRect();
        return {
          left: box.left,
          top: box.top,
          width: box.width,
          height: box.height,
          focused: document.activeElement === element,
        };
      }, selector);
      if (!rect?.focused) {
        problems.push(`${selector}: cannot take the focus`);
        continue;
      }
      const { image: after, scale } = await settledFrame();
      const pixels = newAccentPixels(before, after, scale, rect, accent);
      // At least a 1.5 px line along the shorter side.
      const needed = 1.5 * Math.min(rect.width, rect.height) * scale * scale;
      if (pixels < needed) {
        problems.push(`${selector}: ${String(pixels)} accent pixels, ${needed.toFixed(0)} needed`);
      }
    }
    expect(problems).toEqual([]);
  });

  it('operates Explorer and tabs with the keyboard', async () => {
    const list = $('#pw-explorer-list');
    await list.click();
    await browser.keys('Home');
    await browser.keys('Enter');
    await browser.waitUntil(async () => (await tabTitles()).includes('Unbenannt'), {
      timeoutMsg: 'Enter did not open the document',
    });
    // F2 renames in place.
    await list.click();
    await browser.keys('Home');
    await browser.keys('F2');
    const input = $('[data-testid="explorer-rename"]');
    await input.waitForDisplayed();
    await browser.keys(['Control', 'a']);
    await browser.keys('Erstes Kapitel'.split(''));
    await browser.keys('Enter');
    await expect($('[data-testid="explorer"]')).toHaveText(expect.stringContaining('Erstes Kapitel'));
    // Context menu with Shift+F10, closed with Escape.
    await list.click();
    await browser.keys(['Shift', 'F10']);
    await expect($('[data-testid="context-menu"]')).toBeDisplayed();
    await browser.keys('Escape');
    await expect($('[data-testid="context-menu"]')).not.toBeExisting();
    // Tabs: arrow keys move, Enter activates, Ctrl+Shift+PageUp moves the tab.
    const before = await tabTitles();
    expect(before.length).toBeGreaterThanOrEqual(2);
    await $('[data-testid="tabs"] [role="tab"][aria-selected="true"]').click();
    await browser.keys(['Control', 'Shift', 'PageUp']);
    const after = await tabTitles();
    expect(after).not.toEqual(before);
    expect([...after].sort()).toEqual([...before].sort());
  });

  it('imports dropped text files and asks before importing the same content again', async () => {
    const drop = async (name: string, content: string, type: string) => {
      await browser.execute(
        (fileName: string, text: string, mime: string) => {
          const data = new DataTransfer();
          data.items.add(new File([text], fileName, { type: mime }));
          const target = document.querySelector('[data-testid="explorer"]') ?? document.body;
          for (const kind of ['dragenter', 'dragover', 'drop']) {
            target.dispatchEvent(
              new DragEvent(kind, { bubbles: true, cancelable: true, dataTransfer: data }),
            );
          }
        },
        name,
        content,
        type,
      );
    };
    await drop('Importiert.md', '# Import\r\nText aus einer Datei\r\n', 'text/markdown');
    await expect($('[data-testid="notice"]')).toHaveText(expect.stringContaining('Importiert: 1'));
    await browser.waitUntil(async () => (await tabTitles()).includes('Importiert'), {
      timeoutMsg: 'the imported document was not opened',
    });
    const id = await activeDocumentId();
    expect(fs.readFileSync(documentFile(id), 'utf8')).toBe('# Import\nText aus einer Datei\n');

    await drop('Kopie.md', '# Import\nText aus einer Datei\n', 'text/markdown');
    const dialog = $('[data-testid="duplicate-dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining('Importiert'));
    await $('[data-testid="duplicate-existing"]').click();
    await expect(dialog).not.toBeExisting();
    expect(await activeDocumentId()).toBe(id);

    await drop('bild.png', '\u0089PNG\r\n\u001a\n\u0000\u0000', 'image/png');
    await expect($('[data-testid="notice"]')).toHaveText(expect.stringContaining('bild.png'));
    const titles = await $('[data-testid="explorer"]').getText();
    expect(titles).not.toContain('Kopie');
    expect(titles).not.toContain('bild');
  });

  it('reorders tabs by drag and drop', async () => {
    const before = await tabTitles();
    expect(before.length).toBeGreaterThanOrEqual(2);
    await browser.execute(() => {
      const tabs = [...document.querySelectorAll<HTMLElement>('[data-testid="tabs"] [role="tab"]')];
      const last = tabs[tabs.length - 1];
      const first = tabs[0];
      const list = document.querySelector('[data-testid="tabs"]');
      if (!last || !first || !list) {
        return;
      }
      const data = new DataTransfer();
      last.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: data }));
      const x = first.getBoundingClientRect().left + 2;
      list.dispatchEvent(
        new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer: data, clientX: x }),
      );
      list.dispatchEvent(
        new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: data, clientX: x }),
      );
    });
    const after = await tabTitles();
    expect(after[0]).toBe(before[before.length - 1]);
    expect([...after].sort()).toEqual([...before].sort());
  });

  it('imports a theme file, checks its contrast and removes it again', async () => {
    const choose = async (name: string, content: string) => {
      await browser.execute(
        (fileName: string, text: string) => {
          const input = document.querySelector<HTMLInputElement>('[data-testid="theme-file"]');
          if (!input) {
            throw new Error('no theme file input');
          }
          const data = new DataTransfer();
          data.items.add(new File([text], fileName, { type: 'application/json' }));
          input.files = data.files;
          input.dispatchEvent(new Event('change', { bubbles: true }));
        },
        name,
        content,
      );
    };
    const colors = {
      surface: '#101820',
      sidebar: '#18222C',
      text: '#E6EDF3',
      textMuted: '#A9B4C0',
      accent: '#7FB2FF',
      error: '#FF9B8A',
      success: '#5FD38A',
      clusterBlue: '#7FB2FF',
      clusterGreen: '#5FD38A',
      clusterViolet: '#C3A0FF',
      clusterOrange: '#F5B060',
      clusterGrey: '#A9B0B8',
    };
    await openSettings('general');
    await choose(
      'zu-blass.json',
      JSON.stringify({
        formatVersion: 1,
        name: 'Blass',
        base: 'dark',
        colors: { ...colors, textMuted: '#3A4450' },
      }),
    );
    await expect($('[data-testid="theme-error"]')).toHaveText(expect.stringContaining('textMuted/surface'));

    await choose('nacht.json', JSON.stringify({ formatVersion: 1, name: 'Nacht', base: 'dark', colors }));
    await browser.waitUntil(
      async () => (await browser.execute(() => document.documentElement.dataset['theme'])) === 'custom',
      {
        timeoutMsg: 'the imported theme was not applied',
      },
    );
    expect(String(readSettings()['theme'])).toMatch(/^custom:[0-9a-f]{32}$/);
    const surface = await browser.execute(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--pw-color-surface').trim(),
    );
    expect(surface.toUpperCase()).toBe('#101820');

    await $('button[aria-label="Design „Nacht“ entfernen"]').click();
    await browser.waitUntil(() => readSettings()['theme'] === 'system', {
      timeoutMsg: 'the theme was not removed',
    });
    await closeSettings();
  });

  it('zooms the interface and changes the side bar, and remembers both', async () => {
    await $('#pw-explorer-list').click();
    await browser.keys(['Control', '=']);
    await expect($('[data-testid="status-zoom"]')).toHaveText(expect.stringContaining('110'));
    await browser.waitUntil(() => readSettings()['zoom'] === 1.1, { timeoutMsg: 'zoom not saved' });
    await browser.keys(['Control', '0']);
    await expect($('[data-testid="status-zoom"]')).not.toBeExisting();

    const separator = $('[data-testid="sidebar-resize"]');
    const width = Number(await separator.getAttribute('aria-valuenow'));
    await separator.click();
    await browser.keys('ArrowRight');
    await browser.waitUntil(
      () => (readSettings()['sidebar'] as { width: number } | undefined)?.width === width + 16,
      { timeoutMsg: 'side bar width not saved' },
    );
    await browser.keys(['Control', 'Shift', 'b']);
    await expect($('[data-testid="sidebar"]')).not.toBeExisting();
    await browser.waitUntil(
      () => (readSettings()['sidebar'] as { visible: boolean } | undefined)?.visible === false,
      { timeoutMsg: 'side bar state not saved' },
    );
    await browser.keys(['Control', 'Shift', 'b']);
    await expect($('[data-testid="sidebar"]')).toBeDisplayed();
  });

  it('switches the interface language at once', async () => {
    await openSettings('general');
    await $('[data-testid="language-en"]').click();
    await expect($('[data-testid="settings-tab-general"]')).toHaveText('General');
    await $('[data-testid="language-de"]').click();
    await expect($('[data-testid="settings-tab-general"]')).toHaveText('Allgemein');
    await closeSettings();
  });
});
