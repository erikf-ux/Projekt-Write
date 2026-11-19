/**
 * Unit tests of the network test's evaluation (scripts/network-check.mjs).
 * Run: node --test scripts/network-check.test.mjs
 *
 * The recording lines are in the format of scripts/network-trace.bt; the
 * bytes of the forbidden examples come from a real recording (bpftrace
 * 0.20.2): a process named project-write that asked systemd-resolved, sent
 * a UDP message to port 53 with sendmsg, connected to a web server from a
 * thread and started `getent hosts`, which asked 8.8.8.8.
 */
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  classify,
  decodeSockaddr,
  evaluateRecording,
  evaluateWindowsSamples,
  formatIpv6,
  formatLinuxReport,
  isLoopback,
  parseRecording,
} from './network-check.mjs';

/** `\xNN` for each byte, as bpftrace's `%rx` prints them. */
function hex(bytes) {
  return bytes.map((byte) => `\\x${byte.toString(16).padStart(2, '0')}`).join('');
}

/** A sockaddr_un with `path` (a leading "@" makes it abstract). */
function unix(path) {
  const name = path.startsWith('@') ? [0, ...Buffer.from(path.slice(1))] : [...Buffer.from(path), 0];
  return [1, 0, ...name];
}

const X11 = unix('@/tmp/.X11-unix/X99');
const DBUS = unix('/tmp/dbus-AbCdEf');
const NETLINK = [16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const WEBDRIVER = [2, 0, 0x11, 0x5d, 127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
const LOOPBACK6 = [10, 0, 0x11, 0x5d, 0, 0, 0, 0, ...new Array(15).fill(0), 1, 0, 0, 0, 0];

const ALLOWED = [
  'ready',
  '4711 exec /work/src-tauri/target/debug/project-write',
  `4711 connect ${String(X11.length)} ${hex(X11)} project-write`,
  `4711 connect ${String(DBUS.length)} ${hex(DBUS)} project-write`,
  `4712 sendmsg 12 ${hex(NETLINK)} WebKitWebProces`,
  `4712 connect 16 ${hex(WEBDRIVER)} pool-spawner`,
  `4712 connect 28 ${hex(LOOPBACK6)} JSC Heap Collector`,
  `4712 connect 2 ${hex([0, 0])} WebKitNetworkPr`,
];

const FORBIDDEN = [
  'ready',
  '26751 exec /tmp/bt/project-write',
  '26751 connect 42 \\x01\\x00\\x2f\\x72\\x75\\x6e\\x2f\\x73\\x79\\x73\\x74\\x65\\x6d\\x64\\x2f\\x72\\x65\\x73\\x6f\\x6c\\x76\\x65\\x2f\\x69\\x6f\\x2e\\x73\\x79\\x73\\x74\\x65\\x6d\\x64\\x2e\\x52\\x65\\x73\\x6f\\x6c\\x76\\x65\\x00 project-write',
  '26751 sendto 16 \\x02\\x00\\x00\\x09\\x7f\\x00\\x00\\x01\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00 project-write',
  '26751 sendmsg 16 \\x02\\x00\\x00\\x35\\x0a\\x01\\x02\\x03\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00 project-write',
  '26751 connect 16 \\x02\\x00\\x01\\xbb\\x5d\\xb8\\xd7\\x0e\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00 project-write',
  '26754 connect 16 \\x02\\x00\\x00\\x35\\x08\\x08\\x08\\x08\\xa0\\x4f\\x40\\x79\\x27\\x7f\\x00\\x00 getent',
];

describe('isLoopback', () => {
  it('accepts 127.0.0.0/8, ::1 and mapped loopback', () => {
    for (const address of ['127.0.0.1', '127.255.0.9', '::1', '0:0:0:0:0:0:0:1', '::ffff:127.0.0.1']) {
      assert.equal(isLoopback(address), true, address);
    }
  });

  it('refuses other addresses', () => {
    for (const address of [
      '8.8.8.8',
      '10.0.0.1',
      '0.0.0.0',
      '::',
      '::ffff:93.184.215.14',
      '2606:2800::1',
      '128.0.0.1',
    ]) {
      assert.equal(isLoopback(address), false, address);
    }
  });
});

describe('formatIpv6', () => {
  it('shortens the longest run of zero groups', () => {
    assert.equal(formatIpv6([...new Array(15).fill(0), 1]), '::1');
    assert.equal(formatIpv6(new Array(16).fill(0)), '::');
    assert.equal(
      formatIpv6([
        0x26, 0x06, 0x28, 0, 0x02, 0x1f, 0xcb, 0x07, 0x68, 0x20, 0x80, 0xda, 0xaf, 0x6b, 0x8b, 0x2c,
      ]),
      '2606:2800:21f:cb07:6820:80da:af6b:8b2c',
    );
    assert.equal(formatIpv6([0xfe, 0x80, ...new Array(13).fill(0), 1]), 'fe80::1');
  });

  it('writes IPv4-mapped addresses with dots', () => {
    assert.equal(formatIpv6([...new Array(10).fill(0), 0xff, 0xff, 127, 0, 0, 1]), '::ffff:127.0.0.1');
  });
});

describe('decodeSockaddr', () => {
  it('reads IPv4, IPv6, Unix and abstract addresses', () => {
    assert.deepEqual(decodeSockaddr(WEBDRIVER), { family: 'AF_INET', port: 4445, address: '127.0.0.1' });
    assert.deepEqual(decodeSockaddr(LOOPBACK6), { family: 'AF_INET6', port: 4445, address: '::1' });
    assert.deepEqual(decodeSockaddr(DBUS), { family: 'AF_UNIX', path: '/tmp/dbus-AbCdEf' });
    assert.deepEqual(decodeSockaddr(X11), { family: 'AF_UNIX', path: '@/tmp/.X11-unix/X99' });
    assert.deepEqual(decodeSockaddr([1, 0]), { family: 'AF_UNIX' });
  });

  it('keeps an unknown family as its number, which is not allowed', () => {
    const address = decodeSockaddr([44, 0, 1, 2]);
    assert.equal(address.family, '44');
    assert.equal(classify(address).allowed, false);
    assert.equal(classify(decodeSockaddr([17, 0, 0, 3])).kind, 'other-family');
  });
});

describe('classify', () => {
  it('allows local sockets, netlink, AF_UNSPEC and loopback', () => {
    const { records } = parseRecording(ALLOWED.join('\n'));
    assert.deepEqual(
      records.map((record) => [record.kind, record.allowed]),
      [
        ['unix', true],
        ['unix', true],
        ['netlink', true],
        ['loopback', true],
        ['loopback', true],
        ['unspec', true],
      ],
    );
  });

  it('treats DNS as forbidden, on loopback too (systemd stub resolver)', () => {
    assert.deepEqual(classify({ family: 'AF_INET', port: 53, address: '127.0.0.53' }), {
      allowed: false,
      kind: 'dns',
    });
    assert.deepEqual(classify({ family: 'AF_INET6', port: 5353, address: 'ff02::fb' }), {
      allowed: false,
      kind: 'dns',
    });
    assert.deepEqual(classify({ family: 'AF_UNIX', path: '/run/systemd/resolve/io.systemd.Resolve' }), {
      allowed: false,
      kind: 'dns',
    });
  });

  it('treats every other address as remote', () => {
    assert.deepEqual(classify({ family: 'AF_INET', port: 443, address: '93.184.215.14' }), {
      allowed: false,
      kind: 'remote',
    });
    assert.deepEqual(classify({ family: 'AF_INET', port: 80, address: '0.0.0.0' }), {
      allowed: false,
      kind: 'remote',
    });
  });
});

describe('parseRecording', () => {
  it('reads starts, calls with the thread name, and lost events', () => {
    const recording = parseRecording([...ALLOWED, 'Lost 3 events', 'Attaching 9 probes...'].join('\n'));
    assert.equal(recording.ready, true);
    assert.deepEqual(recording.starts, [{ pid: 4711, file: '/work/src-tauri/target/debug/project-write' }]);
    assert.equal(recording.records.length, 6);
    assert.equal(recording.records[4]?.comm, 'JSC Heap Collector');
    assert.deepEqual(recording.lost, ['Lost 3 events']);
  });
});

describe('evaluateRecording', () => {
  it('passes a recording with only allowed addresses', () => {
    const summary = evaluateRecording(parseRecording(ALLOWED.join('\n')));
    assert.equal(summary.passed, true);
    assert.equal(summary.findings.length, 0);
    assert.match(formatLinuxReport(summary), /PASSED/);
  });

  it('fails on DNS and remote addresses, from threads and child processes too', () => {
    const summary = evaluateRecording(parseRecording(FORBIDDEN.join('\n')));
    assert.equal(summary.passed, false);
    assert.deepEqual(
      summary.findings.map((finding) => [finding.syscall, finding.kind, finding.endpoint]),
      [
        ['connect', 'dns', '/run/systemd/resolve/io.systemd.Resolve'],
        ['sendmsg', 'dns', '10.1.2.3:53'],
        ['connect', 'remote', '93.184.215.14:443'],
        ['connect', 'dns', '8.8.8.8:53'],
      ],
    );
    assert.match(formatLinuxReport(summary), /line 7: pid 26754 \(getent\) connect dns 8\.8\.8\.8:53/);
  });

  it('fails when the recording is incomplete', () => {
    // No start at all.
    assert.equal(evaluateRecording(parseRecording('ready')).passed, false);
    // Never ready.
    assert.equal(evaluateRecording(parseRecording(ALLOWED.slice(1).join('\n'))).passed, false);
    // Lost events.
    assert.equal(evaluateRecording(parseRecording([...ALLOWED, 'Lost 1 events'].join('\n'))).passed, false);
    // A start without any record.
    const summary = evaluateRecording(parseRecording([...ALLOWED, '4800 exec /x/project-write'].join('\n')));
    assert.equal(summary.passed, false);
    assert.deepEqual(
      summary.silent.map((start) => start.pid),
      [4800],
    );
  });
});

describe('evaluateWindowsSamples', () => {
  const sample = (tcp, udp = []) =>
    JSON.stringify({
      type: 'sample',
      processes: [
        { pid: 10, ppid: 1, name: 'project-write.exe' },
        { pid: 11, ppid: 10, name: 'msedgewebview2.exe' },
      ],
      tcp,
      udp,
    });

  it('passes loopback and listening sockets', () => {
    const text = [
      sample([
        {
          pid: 10,
          localAddress: '127.0.0.1',
          localPort: 4445,
          remoteAddress: '0.0.0.0',
          remotePort: 0,
          state: 'Listen',
        },
        {
          pid: 10,
          localAddress: '127.0.0.1',
          localPort: 4445,
          remoteAddress: '127.0.0.1',
          remotePort: 50000,
          state: 'Established',
        },
      ]),
    ].join('\n');
    const summary = evaluateWindowsSamples(text);
    assert.equal(summary.passed, true);
    assert.equal(summary.loopback.size, 1);
  });

  it('reports remote connections and DNS queries', () => {
    const text = [
      sample([
        {
          pid: 11,
          localAddress: '10.1.0.4',
          localPort: 50001,
          remoteAddress: '13.107.42.16',
          remotePort: 443,
          state: 'Established',
        },
      ]),
      JSON.stringify({ type: 'dns', pid: 11, process: 'msedgewebview2.exe', name: 'edge.microsoft.com' }),
    ].join('\n');
    const summary = evaluateWindowsSamples(text);
    assert.equal(summary.passed, false);
    assert.deepEqual([...summary.remote.keys()], ['msedgewebview2.exe -> 13.107.42.16:443 (Established)']);
    assert.deepEqual([...summary.dns.keys()], ['edge.microsoft.com (msedgewebview2.exe)']);
  });

  it('does not pass when no sample saw the app', () => {
    const summary = evaluateWindowsSamples(
      JSON.stringify({ type: 'sample', processes: [], tcp: [], udp: [] }),
    );
    assert.equal(summary.passed, false);
  });
});
