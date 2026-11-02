#!/usr/bin/env node
/**
 * Regenerates the app icons from src-tauri/icons/app-icon.png, the client's
 * artwork at 256 × 256 px (docs/dependencies.md, section "Grafiken").
 *
 * The largest icon is 256 × 256 px, so the source is never scaled up:
 * * `tauri icon` writes every size into a scratch folder under src-tauri/target/;
 * * 32x32.png, 128x128.png, 128x128@2x.png and icon.ico (16–256 px) are copied;
 * * icon.icns keeps only its entries up to 256 px – the 512 and 1024 px
 *   entries would be enlarged from the 256 px source.
 * The Android, iOS and Microsoft Store variants are not used.
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const iconsDir = path.join(root, 'src-tauri', 'icons');
const source = 'src-tauri/icons/app-icon.png';
const scratch = 'src-tauri/target/app-icons';
const SIZE = 256;
const COPIED = ['32x32.png', '128x128.png', '128x128@2x.png', 'icon.ico'];
// ICNS entry types for 256x256@2x, 512x512 and 512x512@2x.
const OVERSIZED_ICNS_TYPES = new Set(['ic14', 'ic09', 'ic10']);
const PNG_SIGNATURE = 0x89504e47;

/** Width and height of a PNG image. */
function pngSize(data, name) {
  if (data.readUInt32BE(0) !== PNG_SIGNATURE || data.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error(`${name} is not a PNG image`);
  }
  return [data.readUInt32BE(16), data.readUInt32BE(20)];
}

/**
 * The ICNS file without the oversized entries (and without a table of contents).
 * Entries are sorted by type: `tauri icon` writes them in varying order, and a
 * fixed order keeps the file identical between runs.
 */
function icnsUpTo(data, maxSize) {
  if (data.toString('ascii', 0, 4) !== 'icns') {
    throw new Error('icon.icns is not an ICNS file');
  }
  const kept = [];
  for (let offset = 8; offset < data.length;) {
    const type = data.toString('ascii', offset, offset + 4);
    const length = data.readUInt32BE(offset + 4);
    if (length < 8 || offset + length > data.length) {
      throw new Error(`icon.icns: broken entry ${type}`);
    }
    const entry = data.subarray(offset, offset + length);
    offset += length;
    if (OVERSIZED_ICNS_TYPES.has(type) || type === 'TOC ') {
      continue;
    }
    if (entry.length >= 32 && entry.readUInt32BE(8) === PNG_SIGNATURE) {
      const [width] = pngSize(entry.subarray(8), `icon.icns entry ${type}`);
      if (width > maxSize) {
        throw new Error(`icon.icns: unexpected entry ${type} with ${String(width)} px`);
      }
    }
    kept.push({ type, entry });
  }
  const header = Buffer.alloc(8);
  header.write('icns', 0, 'ascii');
  kept.sort((a, b) => (a.type < b.type ? -1 : a.type > b.type ? 1 : 0));
  const body = Buffer.concat(kept.map(({ entry }) => entry));
  header.writeUInt32BE(header.length + body.length, 4);
  return Buffer.concat([header, body]);
}

const [width, height] = pngSize(readFileSync(path.join(root, source)), source);
if (width !== SIZE || height !== SIZE) {
  throw new Error(
    `${source} must be ${String(SIZE)} × ${String(SIZE)} px, found ${String(width)} × ${String(height)}`,
  );
}

rmSync(path.join(root, scratch), { recursive: true, force: true });
// Only npx needs a shell on Windows (npx.cmd); the arguments contain no spaces.
// The CLI's progress output is only shown when it fails (as part of the error).
execFileSync('npx', ['tauri', 'icon', source, '--output', scratch], {
  cwd: root,
  encoding: 'utf8',
  stdio: ['ignore', 'pipe', 'pipe'],
  shell: process.platform === 'win32',
});

for (const name of COPIED) {
  copyFileSync(path.join(root, scratch, name), path.join(iconsDir, name));
}
writeFileSync(
  path.join(iconsDir, 'icon.icns'),
  icnsUpTo(readFileSync(path.join(root, scratch, 'icon.icns')), SIZE),
);
rmSync(path.join(root, scratch), { recursive: true, force: true });

console.log(`App icons written to src-tauri/icons (largest size ${String(SIZE)} × ${String(SIZE)} px).`);
