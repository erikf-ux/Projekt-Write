/**
 * Measurements of the app process for section 6, taken from outside the
 * page:
 *
 * * "Kaltstart bis zur Eingabemöglichkeit" (≤ 2 s): from the moment the
 *   operating system created the app process to the frame in which the
 *   workbench takes input (`pw:ready-for-input`, src/app/readyMark.ts).
 *   The test service starts the app; the WebDriver session begins later,
 *   so the page keeps the mark until the test reads it. Both times are
 *   wall-clock times of the same machine. What the number does not
 *   include: an empty file cache (the binary, WebKitGTK or WebView2 and the
 *   library were read before on the test machine) – a start after a reboot
 *   can take longer.
 * * "Arbeitsspeicher im Leerlauf, ohne LanguageTool" (≤ 500 MB): the
 *   memory of the app process and of every process it started (the
 *   webview processes of WebKitGTK, or msedgewebview2 under Windows).
 *   Linux: proportional set size (PSS, shared pages divided among the
 *   processes that share them) and resident set size (RSS, shared pages
 *   counted in every process) from /proc. Windows: working set and private
 *   bytes from Win32_Process. The target is checked against PSS on Linux
 *   and private bytes on Windows; RSS and the working set count shared
 *   libraries more than once and are logged for comparison.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { browser } from '@wdio/globals';
import { measured } from './measurements';
import { application, sandbox } from './sandbox';

export interface AppProcess {
  pid: number;
  /** When the process was created (ms since the epoch). */
  startedAt: number;
}

export interface ProcessMemory {
  name: string;
  pid: number;
  /** Bytes; PSS on Linux, private bytes on Windows. */
  own: number;
  /** Bytes; RSS on Linux, working set on Windows. */
  resident: number;
}

/** Megabytes as in the target (10^6 bytes). */
const MB = 1_000_000;

function powershell(script: string): string {
  return execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], {
    encoding: 'utf8',
    windowsHide: true,
  });
}

/** The fields of /proc/<pid>/stat after the command name (field 3 on). */
function statFields(pid: number): string[] | null {
  try {
    const stat = fs.readFileSync(`/proc/${String(pid)}/stat`, 'utf8');
    return stat.slice(stat.lastIndexOf(')') + 2).split(' ');
  } catch {
    return null;
  }
}

function linuxPids(): number[] {
  return fs
    .readdirSync('/proc')
    .filter((entry) => /^\d+$/.test(entry))
    .map(Number);
}

let clockTicks: number | undefined;

/** Clock ticks per second of /proc/<pid>/stat (usually 100). */
function ticksPerSecond(): number {
  if (clockTicks === undefined) {
    try {
      clockTicks = Number(execFileSync('getconf', ['CLK_TCK'], { encoding: 'utf8' }).trim()) || 100;
    } catch {
      clockTicks = 100;
    }
  }
  return clockTicks;
}

/** The running app under test (the newest process of its binary). */
export function appProcess(): AppProcess {
  if (process.platform === 'win32') {
    const name = path.basename(application, '.exe');
    const answer = powershell(
      `$p = Get-Process -Name '${name}' | Sort-Object StartTime | Select-Object -Last 1; ` +
        `"$($p.Id) $(([DateTimeOffset]$p.StartTime).ToUnixTimeMilliseconds())"`,
    ).trim();
    const [pid, startedAt] = answer.split(' ').map(Number);
    if (!pid || !startedAt) {
      throw new Error(`the app process was not found: "${answer}"`);
    }
    return { pid, startedAt };
  }
  const binary = fs.realpathSync(application);
  const ticks = ticksPerSecond();
  let newest: { pid: number; start: number } | null = null;
  for (const pid of linuxPids()) {
    let exe: string;
    try {
      exe = fs.readlinkSync(`/proc/${String(pid)}/exe`).replace(/ \(deleted\)$/, '');
    } catch {
      continue;
    }
    const start = Number(statFields(pid)?.[19]);
    if (exe === binary && Number.isFinite(start) && (newest === null || start > newest.start)) {
      newest = { pid, start };
    }
  }
  if (newest === null) {
    throw new Error(`no process of ${binary} is running`);
  }
  // Uptime and the clock read together: the start relative to now.
  const uptime = Number(fs.readFileSync('/proc/uptime', 'utf8').split(' ')[0]);
  const now = Date.now();
  return { pid: newest.pid, startedAt: now - (uptime - newest.start / ticks) * 1000 };
}

/** The app process and every process it started, with their memory. */
export function processTreeMemory(root: number): ProcessMemory[] {
  if (process.platform === 'win32') {
    // Parents and command lines from CIM, memory from Get-Process (bytes).
    const rows = JSON.parse(
      powershell(
        '$m = @{}; Get-Process | ForEach-Object { $m[$_.Id] = @($_.WorkingSet64, $_.PrivateMemorySize64) }; ' +
          'Get-CimInstance Win32_Process | ForEach-Object { $v = $m[[int]$_.ProcessId]; ' +
          '[pscustomobject]@{ Id = $_.ProcessId; Parent = $_.ParentProcessId; Name = $_.Name; ' +
          'Line = [string]$_.CommandLine; WorkingSet = $v[0]; Private = $v[1] } } | ConvertTo-Json -Compress',
      ),
    ) as {
      Id: number;
      Parent: number;
      Name: string;
      Line: string;
      WorkingSet: number | null;
      Private: number | null;
    }[];
    // The webview processes are children of the app; those that use the
    // WebView2 folder of the tests count too, wherever they hang.
    const webviewData = path.join(sandbox, 'webview2').toLowerCase();
    const tree = new Set([root]);
    for (const row of rows) {
      if (row.Line.toLowerCase().includes(webviewData)) {
        tree.add(row.Id);
      }
    }
    for (let grown = true; grown;) {
      grown = false;
      for (const row of rows) {
        if (tree.has(row.Parent) && !tree.has(row.Id)) {
          tree.add(row.Id);
          grown = true;
        }
      }
    }
    return rows
      .filter((row) => tree.has(row.Id))
      .map((row) => ({
        name: row.Name,
        pid: row.Id,
        own: row.Private ?? 0,
        resident: row.WorkingSet ?? 0,
      }));
  }
  const parents = new Map<number, number>();
  for (const pid of linuxPids()) {
    const parent = Number(statFields(pid)?.[1]);
    if (Number.isFinite(parent)) {
      parents.set(pid, parent);
    }
  }
  const tree = new Set([root]);
  for (let grown = true; grown;) {
    grown = false;
    for (const [pid, parent] of parents) {
      if (tree.has(parent) && !tree.has(pid)) {
        tree.add(pid);
        grown = true;
      }
    }
  }
  const kib = (text: string, field: string) =>
    Number(new RegExp(`^${field}:\\s+(\\d+) kB`, 'm').exec(text)?.[1] ?? 0) * 1024;
  const result: ProcessMemory[] = [];
  for (const pid of tree) {
    try {
      const rollup = fs.readFileSync(`/proc/${String(pid)}/smaps_rollup`, 'utf8');
      const name = fs.readFileSync(`/proc/${String(pid)}/comm`, 'utf8').trim();
      result.push({ name, pid, own: kib(rollup, 'Pss'), resident: kib(rollup, 'Rss') });
    } catch {
      // The process ended in between.
    }
  }
  return result;
}

/**
 * Waits for the mark of the first frame that takes input and returns the
 * time from the start of the process to it.
 */
export async function coldStartMs(): Promise<{ ms: number; process: AppProcess }> {
  await browser.waitUntil(
    () => browser.execute(() => performance.getEntriesByName('pw:ready-for-input').length > 0),
    { timeout: 60_000, timeoutMsg: 'the workbench did not mark that it takes input' },
  );
  const ready = await browser.execute(() => {
    const mark = performance.getEntriesByName('pw:ready-for-input')[0];
    return performance.timeOrigin + (mark?.startTime ?? 0);
  });
  const app = appProcess();
  return { ms: ready - app.startedAt, process: app };
}

/** Sums the memory of the process tree and describes it for the log. */
export function memorySummary(processes: ProcessMemory[]): {
  ownMB: number;
  residentMB: number;
  text: string;
} {
  const own = processes.reduce((sum, entry) => sum + entry.own, 0) / MB;
  const resident = processes.reduce((sum, entry) => sum + entry.resident, 0) / MB;
  const [ownName, residentName] = process.platform === 'win32' ? ['private', 'working set'] : ['PSS', 'RSS'];
  const parts = [...processes]
    .sort((a, b) => b.own - a.own)
    .map((entry) => `${entry.name} ${(entry.own / MB).toFixed(0)}`)
    .join(', ');
  return {
    ownMB: own,
    residentMB: resident,
    text: `${ownName} ${own.toFixed(0)} MB, ${residentName} ${resident.toFixed(0)} MB in ${String(processes.length)} processes (${ownName} in MB: ${parts})`,
  };
}

/** Section 6 targets and the limits a CI machine must stay below (only the trend counts there). */
const COLD_START_TARGET_MS = 2_000;
const COLD_START_LIMIT_MS = 5 * COLD_START_TARGET_MS;
const IDLE_MEMORY_TARGET_MB = 500;
const IDLE_MEMORY_LIMIT_MB = 3 * IDLE_MEMORY_TARGET_MB;
/** Idle: this long after the workbench took input, nothing touched in between. */
export const IDLE_AFTER_START_MS = 10_000;

/**
 * Measures the start of the app process the test service just started
 * and, after a pause without input, the memory of its process tree; logs
 * both with the other measurements (measurements.ts). `label` names the
 * library and the situation; `memory: false` measures the start only.
 */
export async function measureStartAndIdleMemory(label: string, options = { memory: true }): Promise<void> {
  const start = await coldStartMs();
  measured(
    `Cold start to input (${process.platform}, ${label}): ${start.ms.toFixed(0)} ms (target ≤ ${String(COLD_START_TARGET_MS)} ms)`,
  );
  if (!(start.ms > 0 && start.ms < COLD_START_LIMIT_MS)) {
    throw new Error(`cold start ${start.ms.toFixed(0)} ms is outside 0 to ${String(COLD_START_LIMIT_MS)} ms`);
  }
  if (!options.memory) {
    return;
  }
  const elapsed = await browser.execute(() => {
    const mark = performance.getEntriesByName('pw:ready-for-input')[0];
    return performance.now() - (mark?.startTime ?? 0);
  });
  await browser.pause(Math.max(0, IDLE_AFTER_START_MS - elapsed));
  const memory = memorySummary(processTreeMemory(start.process.pid));
  measured(
    `Idle memory without LanguageTool (${process.platform}, ${label}, ${String(IDLE_AFTER_START_MS / 1000)} s after input was possible): ${memory.text} (target ≤ ${String(IDLE_MEMORY_TARGET_MB)} MB)`,
  );
  checkMemory(memory.ownMB);
}

/**
 * The memory of the running app after `idleMs` without input; for the
 * report, e.g. after the load tests (the target is the idle start).
 */
export async function measureMemory(label: string, idleMs = IDLE_AFTER_START_MS): Promise<void> {
  await browser.pause(idleMs);
  const memory = memorySummary(processTreeMemory(appProcess().pid));
  measured(
    `Memory without LanguageTool (${process.platform}, ${label}, after ${String(idleMs / 1000)} s without input): ${memory.text}`,
  );
  checkMemory(memory.ownMB);
}

function checkMemory(ownMB: number): void {
  if (ownMB > IDLE_MEMORY_LIMIT_MB) {
    throw new Error(`memory ${ownMB.toFixed(0)} MB is above ${String(IDLE_MEMORY_LIMIT_MB)} MB`);
  }
}
