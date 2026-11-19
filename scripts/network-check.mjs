#!/usr/bin/env node
/**
 * Network test (master prompt, sections 2 and 20; Phase 14: "Der
 * Netzwerktest zeigt im Normalbetrieb keine Verbindung"). There is no update
 * check (ADR 0028), so in normal operation the app may make no network
 * connection at all. The end-to-end suite is the normal operation: it
 * opens libraries, edits, searches, imports media, exports, prints, uses the
 * mesh and the language check.
 *
 * * `linux <file>`: evaluates the recording of `scripts/network-trace.bt`
 *   (bpftrace, started by `scripts/network-record-linux.sh`): every socket
 *   address the app or a process it started passed to `connect`, `sendto`,
 *   `sendmsg` or `sendmmsg`. Allowed are local sockets (AF_UNIX), netlink,
 *   AF_UNSPEC (a socket disconnecting) and loopback (127.0.0.0/8, ::1).
 *   Everything else fails the test: any other IPv4 or IPv6 address, any DNS
 *   query (port 53 or mDNS 5353, loopback included: 127.0.0.53 is the
 *   systemd stub resolver), the local name resolver sockets of
 *   systemd-resolved and Avahi, and any other address family. It also fails
 *   without an app start, when an app start recorded nothing at all and
 *   when bpftrace lost events. Exit code 1 then.
 * * `windows <file>`: evaluates the samples of
 *   `scripts/network-watch-windows.ps1` (TCP connections and UDP endpoints
 *   of the app's process tree, DNS queries of the DNS client log). Samples
 *   miss short connections, so this part only reports; `--strict` makes a
 *   finding fail.
 *
 * The functions are exported for scripts/network-check.test.mjs.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** Unix sockets that resolve host names (nss-resolve, nss-mdns). */
const RESOLVER_SOCKETS = [/\/run\/systemd\/resolve\/io\.systemd\.Resolve/, /avahi-daemon\/socket/];

/** DNS and multicast DNS. */
const DNS_PORTS = new Set([53, 5353]);

/** Address families by number (Linux). */
const FAMILIES = new Map([
  [0, 'AF_UNSPEC'],
  [1, 'AF_UNIX'],
  [2, 'AF_INET'],
  [10, 'AF_INET6'],
  [16, 'AF_NETLINK'],
  [17, 'AF_PACKET'],
]);

/** True for 127.0.0.0/8, ::1 and IPv4-mapped loopback (::ffff:127.x.x.x). */
export function isLoopback(address) {
  const value = address.toLowerCase();
  if (/^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(value)) {
    return true;
  }
  if (value === '::1' || value === '0:0:0:0:0:0:0:1') {
    return true;
  }
  return /^(::ffff:|0:0:0:0:0:ffff:)127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(value);
}

/** An IPv6 address in its short form (RFC 5952), IPv4-mapped as `::ffff:a.b.c.d`. */
export function formatIpv6(bytes) {
  if (bytes.slice(0, 10).every((byte) => byte === 0) && bytes[10] === 0xff && bytes[11] === 0xff) {
    return `::ffff:${bytes.slice(12, 16).join('.')}`;
  }
  const groups = [];
  for (let i = 0; i < 16; i += 2) {
    groups.push(((bytes[i] ?? 0) << 8) | (bytes[i + 1] ?? 0));
  }
  // The longest run of at least two zero groups becomes "::".
  let best = { start: -1, length: 0 };
  for (let i = 0; i < 8;) {
    if (groups[i] !== 0) {
      i += 1;
      continue;
    }
    let j = i;
    while (j < 8 && groups[j] === 0) {
      j += 1;
    }
    if (j - i > best.length && j - i >= 2) {
      best = { start: i, length: j - i };
    }
    i = j;
  }
  const hex = groups.map((group) => group.toString(16));
  if (best.start < 0) {
    return hex.join(':');
  }
  const head = hex.slice(0, best.start).join(':');
  const tail = hex.slice(best.start + best.length).join(':');
  return `${head}::${tail}`;
}

/** The text up to the first NUL byte. */
function cString(bytes) {
  const end = bytes.indexOf(0);
  return Buffer.from(end < 0 ? bytes : bytes.slice(0, end)).toString('latin1');
}

/**
 * A socket address from its first bytes (x86-64: the family in host byte
 * order, ports and addresses in network byte order).
 */
export function decodeSockaddr(bytes) {
  const number = (bytes[0] ?? 0) | ((bytes[1] ?? 0) << 8);
  const family = FAMILIES.get(number) ?? String(number);
  const address = { family };
  if (family === 'AF_INET' && bytes.length >= 8) {
    address.port = ((bytes[2] ?? 0) << 8) | (bytes[3] ?? 0);
    address.address = bytes.slice(4, 8).join('.');
  } else if (family === 'AF_INET6' && bytes.length >= 24) {
    address.port = ((bytes[2] ?? 0) << 8) | (bytes[3] ?? 0);
    address.address = formatIpv6(bytes.slice(8, 24));
  } else if (family === 'AF_UNIX' && bytes.length > 2) {
    const name = bytes.slice(2);
    // An abstract name starts with a NUL byte.
    address.path = name[0] === 0 ? `@${cString(name.slice(1))}` : cString(name);
  }
  return address;
}

/** `{ allowed, kind }` for one socket address. */
export function classify(address) {
  switch (address.family) {
    case 'AF_UNIX':
      if (address.path !== undefined && RESOLVER_SOCKETS.some((pattern) => pattern.test(address.path))) {
        return { allowed: false, kind: 'dns' };
      }
      return { allowed: true, kind: 'unix' };
    case 'AF_NETLINK':
      return { allowed: true, kind: 'netlink' };
    case 'AF_UNSPEC':
      return { allowed: true, kind: 'unspec' };
    case 'AF_INET':
    case 'AF_INET6':
      if (address.port !== undefined && DNS_PORTS.has(address.port)) {
        return { allowed: false, kind: 'dns' };
      }
      if (address.address !== undefined && isLoopback(address.address)) {
        return { allowed: true, kind: 'loopback' };
      }
      return { allowed: false, kind: 'remote' };
    default:
      return { allowed: false, kind: 'other-family' };
  }
}

/** `host:port`, `[v6]:port`, a socket path or the family. */
export function endpoint(address) {
  if (address.address !== undefined) {
    const host = address.family === 'AF_INET6' ? `[${address.address}]` : address.address;
    return `${host}:${String(address.port ?? '?')}`;
  }
  if (address.path !== undefined) {
    return address.path;
  }
  return address.family;
}

const EXEC = /^(\d+) exec (.*)$/;
const CALL = /^(\d+) (connect|sendto|sendmsg|sendmmsg) (\d+) ((?:\\x[0-9a-f]{2})*) ?(.*)$/;

/** Reads a recording of network-trace.bt. */
export function parseRecording(text) {
  const starts = [];
  const records = [];
  const lost = [];
  let ready = false;
  text.split('\n').forEach((line, index) => {
    if (line === 'ready') {
      ready = true;
      return;
    }
    if (/^Lost \d+ events/.test(line)) {
      lost.push(line);
      return;
    }
    const exec = EXEC.exec(line);
    if (exec) {
      starts.push({ pid: Number(exec[1]), file: exec[2] ?? '' });
      return;
    }
    const call = CALL.exec(line);
    if (!call) {
      return;
    }
    const bytes = [...(call[4] ?? '').matchAll(/\\x([0-9a-f]{2})/g)].map((match) =>
      parseInt(match[1] ?? '0', 16),
    );
    const address = decodeSockaddr(bytes);
    records.push({
      line: index + 1,
      pid: Number(call[1]),
      syscall: call[2] ?? '?',
      comm: call[5] ?? '',
      address,
      ...classify(address),
    });
  });
  return { ready, starts, records, lost };
}

function count(map, key) {
  map.set(key, (map.get(key) ?? 0) + 1);
}

/** Evaluates a recording (`parseRecording`). */
export function evaluateRecording(recording) {
  const kinds = new Map();
  const unixPaths = new Map();
  const loopback = new Map();
  const processes = new Map();
  const findings = [];
  for (const record of recording.records) {
    count(kinds, record.kind);
    count(processes, record.comm);
    if (record.kind === 'unix') {
      count(unixPaths, record.address.path ?? '(unnamed)');
    } else if (record.kind === 'loopback') {
      count(loopback, endpoint(record.address));
    }
    if (!record.allowed) {
      findings.push({ ...record, endpoint: endpoint(record.address) });
    }
  }
  // Every start connects to the display and D-Bus itself; a start without
  // any record means the recorder did not see that process.
  const seen = new Set(recording.records.map((record) => record.pid));
  const silent = recording.starts.filter((start) => !seen.has(start.pid));
  return {
    ready: recording.ready,
    starts: recording.starts.length,
    silent,
    lost: recording.lost,
    kinds,
    processes,
    unixPaths,
    loopback,
    findings,
    passed:
      recording.ready &&
      recording.starts.length > 0 &&
      silent.length === 0 &&
      recording.lost.length === 0 &&
      findings.length === 0,
  };
}

function sorted(map) {
  return [...map.entries()].sort((a, b) => b[1] - a[1] || String(a[0]).localeCompare(String(b[0])));
}

function listing(out, title, map) {
  if (map.size === 0) {
    return;
  }
  out.push(title);
  for (const [name, n] of sorted(map)) {
    out.push(`  ${String(n).padStart(7)}  ${name}`);
  }
}

/** A readable report of `evaluateRecording`. */
export function formatLinuxReport(summary) {
  const out = [];
  out.push(`Network test (Linux): ${String(summary.starts)} app start(s) recorded`);
  out.push(
    `Socket addresses by kind: ${
      sorted(summary.kinds)
        .map(([kind, n]) => `${kind} ${String(n)}`)
        .join(', ') || 'none'
    }`,
  );
  listing(out, 'By process (thread name):', summary.processes);
  listing(out, 'Local sockets (AF_UNIX):', summary.unixPaths);
  listing(out, 'Loopback:', summary.loopback);
  if (!summary.ready) {
    out.push('FAILED: the recording is incomplete (bpftrace never reported "ready").');
  }
  if (summary.starts === 0) {
    out.push('FAILED: no app start recorded.');
  }
  for (const start of summary.silent) {
    out.push(
      `FAILED: app start pid ${String(start.pid)} recorded no socket address; the recorder missed it.`,
    );
  }
  for (const line of summary.lost) {
    out.push(`FAILED: bpftrace: ${line}`);
  }
  if (summary.findings.length > 0) {
    out.push(`FAILED: ${String(summary.findings.length)} forbidden connection attempt(s):`);
    for (const finding of summary.findings) {
      out.push(
        `  line ${String(finding.line)}: pid ${String(finding.pid)} (${finding.comm}) ${finding.syscall} ${finding.kind} ${finding.endpoint}`,
      );
    }
  }
  if (summary.passed) {
    out.push('PASSED: no network connection (only local sockets, netlink and loopback).');
  }
  return out.join('\n');
}

/** States in which a TCP entry has no remote end. */
const TCP_WITHOUT_PEER = new Set(['Listen', 'Bound', 'Closed']);

/** True for the unspecified address (0.0.0.0, ::). */
function isUnspecified(address) {
  return address === '0.0.0.0' || address === '::' || address === '';
}

/**
 * Evaluates the samples of network-watch-windows.ps1 (one JSON object per
 * line: `sample` with processes, tcp, udp; `dns` with a query of the app).
 */
export function evaluateWindowsSamples(text) {
  let samples = 0;
  let samplesWithApp = 0;
  const processNames = new Map();
  const remote = new Map();
  const loopback = new Map();
  const udp = new Map();
  const dns = new Map();
  for (const line of text.split('\n')) {
    if (line.trim() === '') {
      continue;
    }
    const entry = JSON.parse(line);
    if (entry.type === 'dns') {
      count(dns, `${String(entry.name)} (${String(entry.process ?? entry.pid)})`);
      continue;
    }
    if (entry.type !== 'sample') {
      continue;
    }
    samples += 1;
    const names = new Map();
    for (const process of entry.processes ?? []) {
      names.set(process.pid, process.name);
      count(processNames, process.name);
    }
    if (names.size > 0) {
      samplesWithApp += 1;
    }
    for (const connection of entry.tcp ?? []) {
      if (TCP_WITHOUT_PEER.has(connection.state) || isUnspecified(connection.remoteAddress)) {
        continue;
      }
      const name = names.get(connection.pid) ?? String(connection.pid);
      const where = `${name} -> ${String(connection.remoteAddress)}:${String(connection.remotePort)}`;
      if (isLoopback(connection.remoteAddress) && !DNS_PORTS.has(connection.remotePort)) {
        count(loopback, where);
      } else {
        count(remote, `${where} (${String(connection.state)})`);
      }
    }
    for (const endpointEntry of entry.udp ?? []) {
      const name = names.get(endpointEntry.pid) ?? String(endpointEntry.pid);
      count(udp, `${name} ${String(endpointEntry.localAddress)}:${String(endpointEntry.localPort)}`);
    }
  }
  return {
    samples,
    samplesWithApp,
    processNames,
    remote,
    loopback,
    udp,
    dns,
    passed: samplesWithApp > 0 && remote.size === 0 && dns.size === 0,
  };
}

/** A readable report of `evaluateWindowsSamples`. */
export function formatWindowsReport(summary) {
  const out = [];
  out.push(
    `Network test (Windows, sampled): ${String(summary.samples)} samples, ${String(summary.samplesWithApp)} with the app running`,
  );
  const section = (title, map) => {
    if (map.size === 0) {
      return;
    }
    out.push(title);
    for (const [name, n] of sorted(map)) {
      out.push(`  ${String(n).padStart(7)}  ${name}`);
    }
  };
  section('Processes of the app (samples):', summary.processNames);
  section('Loopback TCP:', summary.loopback);
  section('UDP endpoints (local address only; UDP has no connection to show):', summary.udp);
  section('Remote TCP connections:', summary.remote);
  section('DNS queries of the app (DNS client log):', summary.dns);
  if (summary.samplesWithApp === 0) {
    out.push('NOT EVALUATED: no sample saw the app.');
  } else if (summary.passed) {
    out.push('No remote connection and no DNS query seen (samples can miss short connections).');
  } else {
    out.push('FOUND: the app process tree made remote connections or DNS queries (see above).');
  }
  return out.join('\n');
}

function annotate(level, message) {
  if (process.env['GITHUB_ACTIONS'] === 'true') {
    console.log(`::${level}::${message}`);
  }
}

function main(argv) {
  const [mode, target, ...flags] = argv;
  if (mode === 'linux' && target) {
    const read = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '');
    // bpftrace reports lost events on stderr (network-record-linux.sh keeps it as .log).
    const lost = read(`${target}.log`)
      .split('\n')
      .filter((line) => /^Lost \d+ events/.test(line));
    const summary = evaluateRecording(parseRecording([read(target), ...lost].join('\n')));
    console.log(formatLinuxReport(summary));
    if (!summary.passed) {
      annotate(
        'error',
        'Network test (Linux): a forbidden connection attempt, or the recording is incomplete',
      );
      return 1;
    }
    return 0;
  }
  if (mode === 'windows' && target) {
    const summary = evaluateWindowsSamples(fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '');
    console.log(formatWindowsReport(summary));
    if (!summary.passed) {
      annotate(
        'warning',
        summary.samplesWithApp === 0
          ? 'Network test (Windows): no sample saw the app'
          : 'Network test (Windows): remote connections or DNS queries of the app process tree',
      );
      return flags.includes('--strict') ? 1 : 0;
    }
    return 0;
  }
  console.error('usage: network-check.mjs linux <recording> | windows <samples-file> [--strict]');
  return 2;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
