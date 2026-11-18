#!/usr/bin/env node
/**
 * Writes src-tauri/installer/license.txt, the one text the license pages of
 * the Windows installers (NSIS, MSI) and the Debian copyright file show.
 * Project Write is licensed under MIT OR Apache-2.0 (ADR 0028); an installer
 * shows a single file, so this one states the choice and contains both texts
 * verbatim. The two files in the root stay the source.
 *
 *   node scripts/installer-license.mjs          write the file
 *   node scripts/installer-license.mjs --check  fail if it is out of date
 *
 * The text is plain ASCII, so NSIS, WiX (which converts it to RTF) and dpkg
 * show it the same way.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = path.join(root, 'src-tauri', 'installer', 'license.txt');

function read(name) {
  // Line endings are normalised, so a checkout with CRLF gives the same text.
  return readFileSync(path.join(root, name), 'utf8').replace(/\r\n/g, '\n').trimEnd();
}

export function installerLicense() {
  const rule = '='.repeat(78);
  return [
    'Project Write',
    '',
    'Copyright (c) 2026 The Project Write authors',
    '',
    'Project Write is licensed under either of',
    '',
    '  * the MIT License (LICENSE-MIT), or',
    '  * the Apache License, Version 2.0 (LICENSE-APACHE),',
    '',
    'at your option (SPDX: MIT OR Apache-2.0). Both texts follow below.',
    '',
    'Third-party components keep their own licences. They are listed in the',
    'file THIRD-PARTY-LICENSES in the installation folder; every dictionary',
    'comes with its licence text in dictionaries/<language>/LICENSE.',
    '',
    rule,
    'MIT License (LICENSE-MIT)',
    rule,
    '',
    read('LICENSE-MIT'),
    '',
    rule,
    'Apache License, Version 2.0 (LICENSE-APACHE)',
    rule,
    '',
    read('LICENSE-APACHE'),
    '',
  ].join('\n');
}

function main() {
  const text = installerLicense();
  if (/[^\t\n\x20-\x7e]/.test(text)) {
    throw new Error('the installer license must be plain ASCII');
  }
  if (process.argv.includes('--check')) {
    let current = '';
    try {
      current = readFileSync(target, 'utf8').replace(/\r\n/g, '\n');
    } catch {
      // Missing counts as out of date.
    }
    if (current !== text) {
      console.error(`${path.relative(root, target)} is out of date: run node scripts/installer-license.mjs`);
      process.exit(1);
    }
    console.log(`${path.relative(root, target)} is up to date`);
    return;
  }
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, text);
  console.log(`wrote ${path.relative(root, target)}`);
}

main();
