#!/usr/bin/env node
/**
 * Builds the reproducible reference library (master prompt, section 6) with
 * the Rust example in src-tauri/examples/reference_library.
 *
 *   node scripts/reference-library.mjs [--out <folder>] [--seed <n>] [--documents <n>] [--force]
 *   node scripts/reference-library.mjs --check [--seed <n>] [--documents <n>]
 *
 * * The default folder is src-tauri/target/reference-library. The target must
 *   lie inside the project folder (section 2); the example refuses anything
 *   else and replaces an existing folder only with --force and only if it is
 *   a reference library itself.
 * * --check builds the library twice, each time into a new folder below
 *   src-tauri/target that this run creates (reference-check-XXXXXX/a and /b),
 *   fails if the fingerprints differ, and removes only that folder again.
 *   Folders of earlier runs (for example restored from a CI cache) are never
 *   touched.
 *
 * Phase 2 holds documents only (10,000 by default, one with 50,000 words).
 * Media, subnets, tags and connections follow with their tables.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = path.join(root, 'src-tauri', 'target');
const binary = path.join(
  target,
  'debug',
  'examples',
  process.platform === 'win32' ? 'reference_library.exe' : 'reference_library',
);

function parse(argv) {
  const options = { out: path.join(target, 'reference-library'), seed: '2026', documents: '10000' };
  const flags = new Set();
  for (let i = 0; i < argv.length; i++) {
    const argument = argv[i];
    if (argument === '--force' || argument === '--check') {
      flags.add(argument);
    } else if (['--out', '--seed', '--documents'].includes(argument) && i + 1 < argv.length) {
      options[argument.slice(2)] = argv[++i];
    } else {
      throw new Error(`unknown or incomplete argument: ${argument}`);
    }
  }
  return { ...options, force: flags.has('--force'), check: flags.has('--check') };
}

function build() {
  // cargo runs without a shell, so no argument is ever interpreted.
  execFileSync(
    'cargo',
    [
      'build',
      '--manifest-path',
      path.join(root, 'src-tauri', 'Cargo.toml'),
      '--example',
      'reference_library',
    ],
    { stdio: 'inherit' },
  );
}

/** Runs the example; returns its fingerprint. */
function generate(out, seed, documents, force) {
  const args = ['--out', out, '--seed', seed, '--documents', documents];
  if (force) {
    args.push('--force');
  }
  const output = execFileSync(binary, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
  process.stdout.write(output);
  const match = /^fingerprint: ([0-9a-f]{64})$/m.exec(output);
  if (!match) {
    throw new Error('the example printed no fingerprint');
  }
  return match[1];
}

const options = parse(process.argv.slice(2));
build();
if (options.check) {
  // A folder of this run only: nothing that existed before is overwritten.
  const work = fs.mkdtempSync(path.join(target, 'reference-check-'));
  let first;
  let second;
  try {
    first = generate(path.join(work, 'a'), options.seed, options.documents, false);
    second = generate(path.join(work, 'b'), options.seed, options.documents, false);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
  if (first !== second) {
    console.error(`not reproducible: ${first} != ${second}`);
    process.exit(1);
  }
  console.log(`reproducible: both builds have fingerprint ${first}`);
} else {
  generate(path.resolve(options.out), options.seed, options.documents, options.force);
}
