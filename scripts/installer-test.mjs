#!/usr/bin/env node
/**
 * Installer checks of Phase 14 (master prompt, sections 6, 20 and 25; ADR 0028).
 *
 *   node scripts/installer-test.mjs size <bundle folder> [--limit-mb 50] [--report-only AppImage]
 *   node scripts/installer-test.mjs nsis|msi|deb|appimage <file or folder> [--no-smoke]
 *
 * `size` prints the size of every installer below the folder and fails if one
 * is larger than the limit ("Installationspaket ohne Zusatzpakete ≤ 50 MB",
 * 1 MB = 1,000,000 bytes, the stricter reading). Kinds named with
 * --report-only are measured and printed but do not fail: the AppImage
 * carries WebKitGTK with ICU (about 150 MB unpacked), which the other packages
 * take from the system (docs/installation.md, "Paketgrößen").
 *
 * The four package commands run the same sequence on a clean system (a fresh
 * CI runner) and stop at the first failure:
 *
 *   1. Clean system: the app is not installed and there is no library folder
 *      at the default location (`Documents/ProjectWrite`, section 9).
 *   2. Silent installation (NSIS `/S`, `msiexec /qn`, `apt-get install`; an
 *      AppImage is copied into ~/Applications, it has no installer).
 *   3. The installed files: the program, every dictionary file byte for byte
 *      as in src-tauri/dictionaries (ADR 0024), LICENSE-MIT, LICENSE-APACHE
 *      THIRD-PARTY-LICENSES and THIRD-PARTY-NOTICES byte for byte as in the repository, and no
 *      test plugin in the installed program (section 20).
 *   4. Smoke start (unless --no-smoke): the installed program starts, is
 *      still running after a few seconds and has written its start line to
 *      the log; then it is ended. Linux runs it in xvfb with a D-Bus session.
 *   5. A library at the default location: manifest, SQLite file, documents
 *      with non-ASCII names, a medium and an empty folder. A snapshot records
 *      path, size, SHA-256 and modification time of every entry.
 *   6. Silent uninstallation (NSIS uninstaller `/S`, `msiexec /x /qn`,
 *      `apt-get purge`; the AppImage file is deleted).
 *   7. The program files are gone: installation folder, program, resources,
 *      uninstall entry, shortcuts (Windows) or every file of the package
 *      (Linux, from `dpkg -L`).
 *   8. The library is untouched: the snapshot is identical, byte for byte.
 *
 * The library placeholder is removed at the end, so the next package can be
 * tested on the same runner from a clean state again. The script refuses to
 * run if a library folder already exists: it never touches real data.
 */
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
  appendFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const config = JSON.parse(readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const PRODUCT = config.productName;
const VERSION = config.version;
const IDENTIFIER = config.identifier;
const LIBRARY_FOLDER = 'ProjectWrite';
const LICENSE_FILES = ['LICENSE-MIT', 'LICENSE-APACHE', 'THIRD-PARTY-LICENSES', 'THIRD-PARTY-NOTICES'];
const TEST_PLUGIN_MARKERS = ['tauri-plugin-wdio', 'wdio-webdriver', 'tauri_plugin_wdio'];
const SMOKE_SECONDS = 15;
const isWindows = process.platform === 'win32';

// --- Small helpers -----------------------------------------------------------

class CheckFailed extends Error {}

function fail(message) {
  throw new CheckFailed(message);
}

function step(title) {
  console.log(`\n== ${title}`);
}

function ok(message) {
  console.log(`   ok: ${message}`);
}

function warn(message) {
  console.log(`::warning::${message}`);
}

/** Runs a program without a shell; arguments are never interpreted. */
function run(command, args, { allowFailure = false, quiet = false, env, cwd } = {}) {
  if (!quiet) {
    console.log(`   $ ${[command, ...args].join(' ')}`);
  }
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    env: env ?? process.env,
    cwd,
  });
  if (result.error) {
    fail(`${command} could not be started: ${result.error.message}`);
  }
  if (result.status !== 0 && !allowFailure) {
    fail(`${command} ended with ${result.status}\n${result.stdout}\n${result.stderr}`);
  }
  return result;
}

function sha256(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(condition, { seconds, what }) {
  const end = Date.now() + seconds * 1000;
  while (Date.now() < end) {
    if (condition()) {
      return;
    }
    await sleep(1000);
  }
  if (!condition()) {
    fail(`timed out after ${seconds} s waiting for: ${what}`);
  }
}

/** All files below `dir`, as paths relative to it with "/" separators. */
function filesBelow(dir) {
  const out = [];
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else {
        out.push(path.relative(dir, full).split(path.sep).join('/'));
      }
    }
  };
  walk(dir);
  return out.sort();
}

/** Files below `dir` whose name matches `test`, depth first. */
function findFiles(dir, test) {
  if (!existsSync(dir)) {
    return [];
  }
  return filesBelow(dir)
    .filter((relative) => test(path.posix.basename(relative)))
    .map((relative) => path.join(dir, ...relative.split('/')));
}

function megabytes(bytes) {
  return (bytes / 1_000_000).toFixed(1);
}

function summary(markdown) {
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${markdown}\n`);
  }
}

// --- Package sizes -------------------------------------------------------------

const INSTALLER_KINDS = [
  { kind: 'NSIS', test: (name) => name.endsWith('-setup.exe') },
  { kind: 'MSI', test: (name) => name.endsWith('.msi') },
  { kind: 'deb', test: (name) => name.endsWith('.deb') },
  { kind: 'AppImage', test: (name) => name.endsWith('.AppImage') },
];

function installersIn(dir) {
  const found = [];
  for (const { kind, test } of INSTALLER_KINDS) {
    for (const file of findFiles(dir, test)) {
      found.push({ kind, file, bytes: statSync(file).size });
    }
  }
  return found;
}

function sizeCommand(dir, limitMb, reportOnly) {
  const limit = limitMb * 1_000_000;
  const found = installersIn(dir);
  if (found.length === 0) {
    fail(`no installer below ${dir}`);
  }
  step(`Package sizes (limit ${limitMb} MB = ${limit.toLocaleString('en-US')} bytes)`);
  const rows = ['| Paket | Datei | Größe | Grenze |', '| --- | --- | ---: | --- |'];
  let tooLarge = 0;
  for (const { kind, file, bytes } of found) {
    const within = bytes <= limit;
    const enforced = !reportOnly.includes(kind);
    tooLarge += within || !enforced ? 0 : 1;
    const verdict = within ? 'ok  ' : enforced ? 'FAIL' : 'over';
    console.log(
      `   ${verdict} ${kind.padEnd(8)} ${megabytes(bytes).padStart(6)} MB  ` +
        `${bytes.toLocaleString('en-US').padStart(12)} bytes  ${path.basename(file)}`,
    );
    if (!within && !enforced) {
      warn(
        `${kind} is ${megabytes(bytes)} MB, above ${limitMb} MB (measured only, see docs/installation.md)`,
      );
    }
    const mark = within ? '≤' : enforced ? '**>**' : '> (nur gemessen)';
    rows.push(`| ${kind} | ${path.basename(file)} | ${megabytes(bytes)} MB | ${mark} ${limitMb} MB |`);
  }
  summary(`### Installationspakete (${os.platform()})\n\n${rows.join('\n')}\n`);
  if (tooLarge > 0) {
    fail(`${tooLarge} installer(s) larger than ${limitMb} MB (master prompt, section 6)`);
  }
}

// --- The library placeholder ---------------------------------------------------

/** The documents folder the app uses (src-tauri/src/lib.rs, `app_dirs`). */
function documentsDir() {
  if (isWindows) {
    const result = run(
      'powershell',
      ['-NoProfile', '-NonInteractive', '-Command', "[Environment]::GetFolderPath('MyDocuments')"],
      { quiet: true },
    );
    const dir = result.stdout.trim();
    return dir || path.join(os.homedir(), 'Documents');
  }
  // XDG user dirs as the `dirs` crate reads them; without an entry the app
  // falls back to ~/Documents.
  const configHome = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config');
  try {
    const text = readFileSync(path.join(configHome, 'user-dirs.dirs'), 'utf8');
    const match = /^XDG_DOCUMENTS_DIR="([^"]*)"/m.exec(text);
    if (match?.[1]) {
      const dir = match[1].replace(/^\$HOME/, os.homedir());
      if (path.resolve(dir) !== path.resolve(os.homedir())) {
        return dir;
      }
    }
  } catch {
    // No user-dirs.dirs: the fallback below.
  }
  return path.join(os.homedir(), 'Documents');
}

function libraryDir() {
  return path.join(documentsDir(), LIBRARY_FOLDER);
}

/** A small library that looks like a real one (section 9, "Speicherstruktur"). */
function createLibrary(dir) {
  const write = (relative, content) => {
    const file = path.join(dir, ...relative.split('/'));
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
  };
  write(
    'library.json',
    `${JSON.stringify({ format: 1, id: '7f1d3c1e-3b7a-4d3e-9a51-installer-test', name: 'Installationstest' }, null, 2)}\n`,
  );
  // A database file: the SQLite header and deterministic page bytes.
  const pages = Buffer.alloc(8192);
  Buffer.from('SQLite format 3\0', 'latin1').copy(pages);
  for (let i = 16; i < pages.length; i += 1) {
    pages[i] = (i * 31 + 7) % 251;
  }
  write('database/projectwrite.db', pages);
  write(
    'documents/markdown/Kapitel 1 – Übersicht.md',
    '# Übersicht\n\nGrüße aus der Bibliothek. Ein Satz mit ß.\n',
  );
  write('documents/text/notizen.txt', 'Notizen\r\nZeile mit CRLF\r\n');
  write(
    'documents/richtext/brief.pwrite',
    `${JSON.stringify({ format: 'pwrite', version: 1, content: { type: 'doc', content: [] } })}\n`,
  );
  write('media/bild.png', Buffer.from('89504e470d0a1a0a0000000d49484452', 'hex'));
  mkdirSync(path.join(dir, 'backups'), { recursive: true });
}

/** Path, type, size, SHA-256 and modification time of every entry. */
function snapshot(dir) {
  const entries = [];
  const walk = (current) => {
    for (const name of readdirSync(current).sort()) {
      const full = path.join(current, name);
      const stat = lstatSync(full);
      const relative = path.relative(dir, full).split(path.sep).join('/');
      if (stat.isDirectory()) {
        entries.push(`dir  ${relative} mtime=${stat.mtimeMs}`);
        walk(full);
      } else {
        entries.push(`file ${relative} size=${stat.size} sha256=${sha256(full)} mtime=${stat.mtimeMs}`);
      }
    }
  };
  walk(dir);
  return entries;
}

function compareSnapshots(before, after) {
  const removed = before.filter((line) => !after.includes(line));
  const added = after.filter((line) => !before.includes(line));
  if (removed.length > 0 || added.length > 0) {
    fail(
      `the library changed during uninstallation\n  before only:\n    ${removed.join('\n    ')}\n` +
        `  after only:\n    ${added.join('\n    ')}`,
    );
  }
}

// --- Checks of an installation -----------------------------------------------

/** Every shipped file is installed and identical to the repository. */
function verifyInstalledFiles({ binary, resources }) {
  if (!existsSync(binary)) {
    fail(`program not found: ${binary}`);
  }
  ok(`program ${binary} (${megabytes(statSync(binary).size)} MB)`);

  const source = path.join(root, 'src-tauri', 'dictionaries');
  const installed = path.join(resources, 'dictionaries');
  const expected = filesBelow(source);
  for (const relative of expected) {
    const file = path.join(installed, ...relative.split('/'));
    if (!existsSync(file)) {
      fail(`dictionary file missing: ${file}`);
    }
    if (sha256(file) !== sha256(path.join(source, ...relative.split('/')))) {
      fail(`dictionary file differs from the repository: ${file}`);
    }
  }
  const extra = filesBelow(installed).filter((relative) => !expected.includes(relative));
  if (extra.length > 0) {
    fail(`unexpected files in ${installed}: ${extra.join(', ')}`);
  }
  ok(`${expected.length} dictionary files in ${installed}, byte for byte`);

  for (const name of LICENSE_FILES) {
    const file = path.join(resources, name);
    if (!existsSync(file)) {
      fail(`licence file missing: ${file}`);
    }
    if (sha256(file) !== sha256(path.join(root, name))) {
      fail(`licence file differs from the repository: ${file}`);
    }
  }
  ok(`${LICENSE_FILES.join(', ')} in ${resources}, byte for byte`);

  const bytes = readFileSync(binary);
  for (const marker of TEST_PLUGIN_MARKERS) {
    if (bytes.includes(Buffer.from(marker))) {
      fail(`test plugin code (${marker}) in the installed program ${binary} (section 20)`);
    }
  }
  ok('no test plugin in the installed program');
}

/** The log file the app writes (Tauri `app_log_dir`, src-tauri/src/logging.rs). */
function logFile() {
  const base = isWindows
    ? process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local')
    : process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share');
  return path.join(base, IDENTIFIER, 'logs', 'project-write.log');
}

function startLines() {
  try {
    return readFileSync(logFile(), 'utf8')
      .split(/\r?\n/)
      .filter((line) => line.includes(`Project Write ${VERSION} starting`)).length;
  } catch {
    return 0;
  }
}

function stopProcess(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  if (isWindows) {
    // The whole tree: WebView2 runs in child processes that would keep files
    // of the installation folder open.
    spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
  } else {
    try {
      process.kill(-child.pid, 'SIGTERM');
    } catch {
      // Already gone.
    }
  }
}

/** Starts the installed program and checks that it keeps running. */
async function smokeStart(command, args, env = process.env) {
  const before = startLines();
  let launcher = command;
  let launcherArgs = args;
  if (!isWindows) {
    if (
      run('sh', ['-c', 'command -v xvfb-run && command -v dbus-run-session'], {
        allowFailure: true,
        quiet: true,
      }).status !== 0
    ) {
      warn('smoke start skipped: xvfb-run or dbus-run-session is missing');
      return;
    }
    launcher = 'dbus-run-session';
    launcherArgs = ['--', 'xvfb-run', '-a', command, ...args];
  }
  console.log(`   $ ${[launcher, ...launcherArgs].join(' ')}`);
  const output = [];
  const child = spawn(launcher, launcherArgs, {
    env,
    detached: !isWindows,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (chunk) => output.push(chunk));
  child.stderr.on('data', (chunk) => output.push(chunk));
  let spawnError = null;
  child.on('error', (error) => {
    spawnError = error;
  });
  try {
    await sleep(SMOKE_SECONDS * 1000);
    if (spawnError) {
      fail(`the program could not be started: ${spawnError.message}`);
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      fail(
        `the program ended within ${SMOKE_SECONDS} s (exit ${child.exitCode ?? child.signalCode})\n` +
          Buffer.concat(output).toString('utf8'),
      );
    }
    ok(`still running after ${SMOKE_SECONDS} s`);
    if (startLines() <= before) {
      fail(`no start line "Project Write ${VERSION} starting" in ${logFile()}`);
    }
    ok(`start line written to ${logFile()}`);
  } finally {
    stopProcess(child);
    await waitFor(() => child.exitCode !== null || child.signalCode !== null, {
      seconds: 30,
      what: 'the program to end',
    });
    // Windows releases the file handles of the ended processes with a delay.
    await sleep(isWindows ? 5000 : 1000);
  }
}

// --- Windows -------------------------------------------------------------------

function powershell(script) {
  return run('powershell', ['-NoProfile', '-NonInteractive', '-Command', script], {
    quiet: true,
  }).stdout.trim();
}

/** Uninstall entries of the app (per user and per machine). */
function uninstallEntries() {
  const json = powershell(
    `$paths = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',` +
      `'HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',` +
      `'HKLM:\\Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*';` +
      // Windows PowerShell 5.1 has no -AsArray; -InputObject keeps the array.
      `ConvertTo-Json -Compress -InputObject @(Get-ItemProperty -Path $paths -ErrorAction SilentlyContinue | ` +
      `Where-Object { $_.DisplayName -eq '${PRODUCT}' } | ` +
      `Select-Object PSChildName, DisplayName, DisplayVersion, Publisher, InstallLocation, UninstallString)`,
  );
  return json ? JSON.parse(json) : [];
}

/** The end of an msiexec log (UTF-16 with a byte order mark, else UTF-8). */
function msiLogTail(log) {
  try {
    const bytes = readFileSync(log);
    const text = bytes[0] === 0xff && bytes[1] === 0xfe ? bytes.toString('utf16le') : bytes.toString('utf8');
    return text.slice(-4000);
  } catch {
    return '(no log)';
  }
}

function unquote(text) {
  return (text ?? '').trim().replace(/^"(.*)"$/, '$1');
}

/** Shortcuts the installers may create (start menu, desktop). */
function shortcutCandidates() {
  const appData = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
  const programData = process.env.ProgramData || 'C:\\ProgramData';
  const publicDir = process.env.PUBLIC || 'C:\\Users\\Public';
  const menu = path.join('Microsoft', 'Windows', 'Start Menu', 'Programs');
  return [
    path.join(appData, menu, `${PRODUCT}.lnk`),
    path.join(appData, menu, PRODUCT),
    path.join(programData, menu, `${PRODUCT}.lnk`),
    path.join(programData, menu, PRODUCT),
    path.join(os.homedir(), 'Desktop', `${PRODUCT}.lnk`),
    path.join(publicDir, 'Desktop', `${PRODUCT}.lnk`),
  ];
}

/** The ProgId the .pwrite extension points to, or "" if none. */
function pwriteProgId() {
  return powershell(
    `$k = Get-Item -LiteralPath 'Registry::HKEY_CLASSES_ROOT\\.pwrite' -ErrorAction SilentlyContinue; ` +
      `if ($k) { $k.GetValue('') }`,
  );
}

function progIdExists(progId) {
  return (
    powershell(`Test-Path -LiteralPath 'Registry::HKEY_CLASSES_ROOT\\${progId.replace(/'/g, "''")}'`) ===
    'True'
  );
}

function windowsInstallation(kind) {
  const entries = uninstallEntries();
  if (entries.length !== 1) {
    fail(`expected one uninstall entry for ${PRODUCT}, found ${entries.length}`);
  }
  const entry = entries[0];
  ok(
    `uninstall entry ${entry.PSChildName}: ${entry.DisplayName} ${entry.DisplayVersion}, publisher "${entry.Publisher}"`,
  );
  if (entry.DisplayVersion !== VERSION) {
    fail(`uninstall entry has version ${entry.DisplayVersion}, expected ${VERSION}`);
  }
  if (entry.Publisher !== config.bundle.publisher) {
    fail(`uninstall entry has publisher "${entry.Publisher}", expected "${config.bundle.publisher}"`);
  }
  let dir = unquote(entry.InstallLocation);
  if (!dir && kind === 'nsis') {
    dir = path.dirname(unquote(entry.UninstallString));
  }
  if (!dir) {
    dir = path.join(process.env.ProgramFiles || 'C:\\Program Files', PRODUCT);
  }
  dir = dir.replace(/[\\/]+$/, '');
  if (!existsSync(dir)) {
    fail(`installation folder not found: ${dir}`);
  }
  const programs = readdirSync(dir).filter((name) => /\.exe$/i.test(name) && !/^uninstall/i.test(name));
  if (programs.length !== 1) {
    fail(`expected one program in ${dir}, found: ${programs.join(', ') || 'none'}`);
  }
  return { dir, binary: path.join(dir, programs[0]), resources: dir, entry };
}

async function windowsTest(kind, installer, { smoke }) {
  step('Clean system');
  if (uninstallEntries().length > 0) {
    fail(`${PRODUCT} is already installed`);
  }
  ok(`${PRODUCT} is not installed`);
  const library = libraryDir();
  if (existsSync(library)) {
    fail(`a library folder already exists at ${library}; refusing to touch it`);
  }
  ok(`no library at ${library}`);

  step(`Silent installation (${kind.toUpperCase()})`);
  if (kind === 'nsis') {
    run(installer, ['/S']);
  } else {
    const log = path.join(os.tmpdir(), 'project-write-msi-install.log');
    const result = run('msiexec', ['/i', installer, '/qn', '/norestart', '/l*v', log], {
      allowFailure: true,
    });
    if (result.status !== 0 && result.status !== 3010) {
      fail(`msiexec /i ended with ${result.status}; log: ${log}\n${msiLogTail(log)}`);
    }
  }
  const installation = windowsInstallation(kind);
  ok(`installed in ${installation.dir}`);

  step('Installed files');
  verifyInstalledFiles(installation);
  const shortcuts = shortcutCandidates().filter((file) => existsSync(file));
  ok(`shortcuts: ${shortcuts.join(', ') || 'none'}`);
  const progId = pwriteProgId();
  if (progId && progIdExists(progId)) {
    ok(`.pwrite is associated with ${progId}`);
  } else {
    warn(`.pwrite is not associated after the ${kind.toUpperCase()} installation`);
  }

  if (smoke) {
    step('Smoke start');
    await smokeStart(installation.binary, []);
  }

  step(`Library at the default location ${library}`);
  createLibrary(library);
  const before = snapshot(library);
  ok(`${before.length} entries recorded`);

  step(`Silent uninstallation (${kind.toUpperCase()})`);
  if (kind === 'nsis') {
    const uninstaller = path.join(installation.dir, 'uninstall.exe');
    if (!existsSync(uninstaller)) {
      fail(`uninstaller not found: ${uninstaller}`);
    }
    // The NSIS uninstaller copies itself to %TEMP% and returns at once.
    run(uninstaller, ['/S']);
    await waitFor(() => !existsSync(installation.dir) && uninstallEntries().length === 0, {
      seconds: 180,
      what: 'the uninstaller to finish',
    });
  } else {
    const log = path.join(os.tmpdir(), 'project-write-msi-uninstall.log');
    const result = run('msiexec', ['/x', installer, '/qn', '/norestart', '/l*v', log], {
      allowFailure: true,
    });
    if (result.status !== 0 && result.status !== 3010) {
      fail(`msiexec /x ended with ${result.status}; log: ${log}\n${msiLogTail(log)}`);
    }
  }

  step('Program files are gone');
  if (existsSync(installation.dir)) {
    fail(
      `installation folder still exists: ${installation.dir}\n  ${filesBelow(installation.dir).join('\n  ')}`,
    );
  }
  ok(`${installation.dir} removed`);
  if (uninstallEntries().length > 0) {
    fail('the uninstall entry still exists');
  }
  ok('uninstall entry removed');
  const leftover = shortcuts.filter((file) => existsSync(file));
  if (leftover.length > 0) {
    fail(`shortcuts left behind: ${leftover.join(', ')}`);
  }
  ok('shortcuts removed');
  if (progId && progIdExists(progId)) {
    warn(`the file type ${progId} is still registered after uninstallation`);
  }

  return { library, before };
}

// --- Linux ---------------------------------------------------------------------

function debInstallation(pkg) {
  const listed = run('dpkg', ['-L', pkg], { quiet: true })
    .stdout.split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  const files = listed.filter((file) => existsSync(file) && !lstatSync(file).isDirectory());
  const readme = files.find((file) => file.endsWith('/dictionaries/README.md'));
  if (!readme) {
    fail(`the package ${pkg} contains no dictionaries/README.md`);
  }
  const binary = files.find((file) => file.startsWith('/usr/bin/'));
  if (!binary) {
    fail(`the package ${pkg} contains no program in /usr/bin`);
  }
  return { binary, resources: path.dirname(path.dirname(readme)), files };
}

function debInstalled(pkg) {
  const result = run('dpkg-query', ['-W', '-f=${Status}', pkg], { allowFailure: true, quiet: true });
  return result.status === 0 && result.stdout.includes('install ok installed');
}

async function debTest(deb, { smoke }) {
  const pkg = run('dpkg-deb', ['-f', deb, 'Package'], { quiet: true }).stdout.trim();
  console.log(
    `   package ${pkg}, depends: ${run('dpkg-deb', ['-f', deb, 'Depends'], { quiet: true }).stdout.trim()}`,
  );

  step('Clean system');
  if (debInstalled(pkg)) {
    fail(`${pkg} is already installed`);
  }
  ok(`${pkg} is not installed`);
  const library = libraryDir();
  if (existsSync(library)) {
    fail(`a library folder already exists at ${library}; refusing to touch it`);
  }
  ok(`no library at ${library}`);

  step('Silent installation (apt-get install)');
  // apt resolves the declared dependencies (webkit2gtk and the rest) from
  // the archive: on a fresh runner this proves the Depends field.
  run('sudo', [
    'env',
    'DEBIAN_FRONTEND=noninteractive',
    'apt-get',
    'install',
    '-y',
    '--no-install-recommends',
    path.resolve(deb),
  ]);
  if (!debInstalled(pkg)) {
    fail(`${pkg} is not installed after apt-get install`);
  }
  const installation = debInstallation(pkg);
  ok(`${installation.files.length} files installed`);

  step('Installed files');
  verifyInstalledFiles(installation);
  for (const needed of ['/usr/share/applications/', '/usr/share/mime/packages/']) {
    const file = installation.files.find((candidate) => candidate.startsWith(needed));
    if (!file) {
      fail(`no file in ${needed}`);
    }
    ok(file);
  }
  const ldd = run('ldd', [installation.binary], { quiet: true }).stdout;
  if (ldd.includes('not found')) {
    fail(`missing libraries for ${installation.binary}:\n${ldd}`);
  }
  ok('every shared library of the program resolves');

  if (smoke) {
    step('Smoke start');
    await smokeStart(installation.binary, []);
  }

  step(`Library at the default location ${library}`);
  createLibrary(library);
  const before = snapshot(library);
  ok(`${before.length} entries recorded`);

  step('Silent uninstallation (apt-get purge)');
  run('sudo', ['env', 'DEBIAN_FRONTEND=noninteractive', 'apt-get', 'purge', '-y', pkg]);

  step('Program files are gone');
  if (debInstalled(pkg)) {
    fail(`${pkg} is still installed`);
  }
  ok(`${pkg} is not installed`);
  const leftover = installation.files.filter((file) => existsSync(file));
  if (leftover.length > 0) {
    fail(`files of the package left behind:\n  ${leftover.join('\n  ')}`);
  }
  ok(`all ${installation.files.length} files of the package removed`);
  if (existsSync(installation.resources)) {
    fail(`resource folder still exists: ${installation.resources}`);
  }
  ok(`${installation.resources} removed`);

  return { library, before };
}

async function appImageTest(appImage, { smoke }) {
  const applications = path.join(os.homedir(), 'Applications');
  const installed = path.join(applications, path.basename(appImage));

  step('Clean system');
  if (existsSync(installed)) {
    fail(`${installed} already exists`);
  }
  const library = libraryDir();
  if (existsSync(library)) {
    fail(`a library folder already exists at ${library}; refusing to touch it`);
  }
  ok(`no library at ${library}`);

  step('Installation (an AppImage is copied, it has no installer)');
  mkdirSync(applications, { recursive: true });
  copyFileSync(appImage, installed);
  chmodSync(installed, 0o755);
  ok(installed);

  step('Installed files (extracted without FUSE)');
  const scratch = mkdtempSync(path.join(os.tmpdir(), 'project-write-appimage-'));
  try {
    run(installed, ['--appimage-extract'], { cwd: scratch });
    const extracted = path.join(scratch, 'squashfs-root');
    const readme = findFiles(path.join(extracted, 'usr'), (name) => name === 'README.md').find((file) =>
      file.endsWith(`${path.sep}dictionaries${path.sep}README.md`),
    );
    if (!readme) {
      fail('the AppImage contains no dictionaries/README.md');
    }
    const binary = path.join(extracted, 'usr', 'bin', 'project-write');
    verifyInstalledFiles({ binary, resources: path.dirname(path.dirname(readme)) });
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }

  if (smoke) {
    step('Smoke start (the AppImage itself)');
    // Without FUSE the runtime extracts itself to a temporary folder first.
    await smokeStart(installed, [], { ...process.env, APPIMAGE_EXTRACT_AND_RUN: '1' });
    // The runtime removes its copy only when the program ends by itself.
    for (const name of readdirSync(os.tmpdir())) {
      const dir = path.join(os.tmpdir(), name);
      if (
        name.startsWith('appimage_extracted_') &&
        existsSync(path.join(dir, 'usr', 'bin', 'project-write'))
      ) {
        rmSync(dir, { recursive: true, force: true });
      }
    }
  }

  step(`Library at the default location ${library}`);
  createLibrary(library);
  const before = snapshot(library);
  ok(`${before.length} entries recorded`);

  step('Uninstallation (the AppImage file is deleted)');
  rmSync(installed);

  step('Program files are gone');
  if (existsSync(installed)) {
    fail(`${installed} still exists`);
  }
  ok(`${installed} removed`);

  return { library, before };
}

// --- Entry point -----------------------------------------------------------------

const PACKAGE_FILES = {
  nsis: (name) => name.endsWith('-setup.exe'),
  msi: (name) => name.endsWith('.msi'),
  deb: (name) => name.endsWith('.deb'),
  appimage: (name) => name.endsWith('.AppImage'),
};

/** The installer file itself, or the single matching one below a folder. */
function resolveInstaller(kind, given) {
  if (!given) {
    fail(`usage: installer-test.mjs ${kind} <file or folder>`);
  }
  if (statSync(given).isFile()) {
    return path.resolve(given);
  }
  const found = findFiles(given, PACKAGE_FILES[kind]);
  if (found.length !== 1) {
    fail(`expected one ${kind} installer below ${given}, found ${found.length}`);
  }
  return path.resolve(found[0]);
}

async function packageCommand(kind, given, options) {
  const supported = isWindows ? ['nsis', 'msi'] : process.platform === 'linux' ? ['deb', 'appimage'] : [];
  if (!supported.includes(kind)) {
    fail(`${kind} cannot be tested on ${process.platform}`);
  }
  const installer = resolveInstaller(kind, given);
  console.log(`Testing ${path.basename(installer)} (${megabytes(statSync(installer).size)} MB)`);
  const { library, before } =
    kind === 'deb'
      ? await debTest(installer, options)
      : kind === 'appimage'
        ? await appImageTest(installer, options)
        : await windowsTest(kind, installer, options);

  step('The library is untouched');
  if (!existsSync(library)) {
    fail(`the library ${library} was deleted`);
  }
  compareSnapshots(before, snapshot(library));
  ok(`all ${before.length} entries identical (path, size, SHA-256, modification time)`);
  rmSync(library, { recursive: true, force: true });

  summary(
    `### ${kind.toUpperCase()}: Installation und Deinstallation bestanden\n\n` +
      `\`${path.basename(installer)}\`: still installiert, Dateien geprüft, ` +
      `${options.smoke ? 'Programm gestartet, ' : ''}still deinstalliert; ` +
      `die Bibliothek in \`${library}\` blieb unverändert.\n`,
  );
  console.log(`\nPASS ${kind}: installed, checked and uninstalled; the library is untouched`);
}

async function main() {
  const [command, given, ...rest] = process.argv.slice(2);
  if (command === 'size') {
    const option = (name) => {
      const index = rest.indexOf(name);
      return index >= 0 ? rest[index + 1] : undefined;
    };
    const limit = Number(option('--limit-mb') ?? 50);
    const reportOnly = (option('--report-only') ?? '').split(',').filter(Boolean);
    const unknown = reportOnly.filter((kind) => !INSTALLER_KINDS.some((known) => known.kind === kind));
    if (!given || !Number.isFinite(limit) || limit <= 0 || unknown.length > 0) {
      fail(
        'usage: installer-test.mjs size <bundle folder> [--limit-mb 50] [--report-only NSIS,MSI,deb,AppImage]',
      );
    }
    sizeCommand(given, limit, reportOnly);
    return;
  }
  if (command in PACKAGE_FILES) {
    await packageCommand(command, given, { smoke: !rest.includes('--no-smoke') });
    return;
  }
  fail('usage: installer-test.mjs size|nsis|msi|deb|appimage <file or folder> [options]');
}

main().catch((error) => {
  if (error instanceof CheckFailed) {
    console.error(`\n::error::${error.message}`);
  } else {
    console.error(error);
  }
  process.exit(1);
});
