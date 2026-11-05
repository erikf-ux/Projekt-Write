/**
 * Phase 7, "Der Explorer bleibt mit der Referenzbibliothek flüssig", first
 * half: provides the reference library (section 6; ADR 0010 and 0017) and
 * makes it the library of the next start. reference.e2e.ts measures there.
 *
 * Full size on Linux (10,000 documents, 500 subnets, 1,000 tags); Windows
 * writes every file synchronously and gets 2,000 documents with 100
 * subnets and 200 tags, as for the check of the generator in CI. A library
 * built by an earlier run with the same size is used again.
 *
 * Runs after crash-recovered.e2e.ts; ends the app.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { browser, expect } from '@wdio/globals';
import { appDirs, referenceCache, referenceGenerator } from '../sandbox';
import { pinMainWindow, waitUntilAppEnded } from '../session';

export const REFERENCE_DOCUMENTS = process.platform === 'win32' ? 2_000 : 10_000;
const library = path.join(referenceCache, `reference-library-${String(REFERENCE_DOCUMENTS)}`);

/** Whether `library` holds a reference library of this size and seed. */
function usable(): boolean {
  try {
    const manifest = JSON.parse(fs.readFileSync(path.join(library, 'library.json'), 'utf8')) as {
      referenceLibrary?: { seed?: number; documents?: number };
    };
    return (
      manifest.referenceLibrary?.seed === 2026 &&
      manifest.referenceLibrary.documents === REFERENCE_DOCUMENTS &&
      fs.existsSync(path.join(library, 'database', 'projectwrite.db'))
    );
  } catch {
    return false;
  }
}

describe('Reference library for the Explorer (Phase 7)', () => {
  before(async () => {
    await pinMainWindow();
  });

  it('provides the reference library and opens it at the next start', async function () {
    this.timeout(1_200_000);
    if (!usable()) {
      if (!fs.existsSync(referenceGenerator)) {
        throw new Error(
          `${referenceGenerator} is missing; build it with "cargo build --manifest-path src-tauri/Cargo.toml --example reference_library"`,
        );
      }
      fs.mkdirSync(referenceCache, { recursive: true });
      // No shell: the arguments are passed as they are.
      const output = execFileSync(
        referenceGenerator,
        ['--out', library, '--documents', String(REFERENCE_DOCUMENTS), '--force'],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] },
      );
      console.log(output.trim());
      expect(output).toMatch(new RegExp(`documents: +${String(REFERENCE_DOCUMENTS)} `));
    }
    expect(usable()).toBe(true);

    // The app ends; its configuration then names the reference library.
    await browser.closeWindow().catch((error: unknown) => {
      if (!String(error).includes('All window handles were removed')) {
        throw error;
      }
    });
    await waitUntilAppEnded();
    const file = path.join(appDirs, 'config', 'settings.json');
    const settings = JSON.parse(fs.readFileSync(file, 'utf8')) as Record<string, unknown>;
    settings['libraryPath'] = library;
    // The tabs belong to the other library.
    delete settings['session'];
    fs.writeFileSync(file, `${JSON.stringify(settings, null, 2)}\n`);
  });
});
