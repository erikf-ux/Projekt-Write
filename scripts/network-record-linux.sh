#!/bin/sh
# Network test, Linux (docs/sicherheit.md): starts and stops the kernel
# recorder scripts/network-trace.bt around the end-to-end suite.
#
#   scripts/network-record-linux.sh start <recording>
#   (run the E2E suite)
#   scripts/network-record-linux.sh stop <recording>
#   node scripts/network-check.mjs linux <recording>
#
# bpftrace needs root; the script uses sudo unless it already runs as root.
# The app itself runs unchanged: no wrapper, no ptrace.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
command=${1:?usage: $0 start|stop <recording>}
recording=${2:?usage: $0 start|stop <recording>}

as_root() {
  if [ "$(id -u)" = 0 ]; then
    "$@"
  else
    sudo "$@"
  fi
}

case "$command" in
  start)
    mkdir -p "$(dirname "$recording")"
    rm -f "$recording" "$recording.pid" "$recording.log"
    # Tracepoints need tracefs; systemd mounts it, a bare container may not.
    if [ ! -d /sys/kernel/tracing/events ] && [ ! -d /sys/kernel/debug/tracing/events ]; then
      as_root mount -t tracefs nodev /sys/kernel/tracing
    fi
    # Unbuffered, so "ready" and every event reach the file at once.
    # Thread ids are keys of the map of app processes, hence the larger map.
    # No descriptor of the caller stays open (a CI step would wait for it).
    as_root sh -c 'echo $$ > "$1.pid"; exec env BPFTRACE_MAX_MAP_KEYS=65536 bpftrace -q -B none "$2" > "$1" 2> "$1.log"' \
      sh "$recording" "$here/network-trace.bt" < /dev/null > /dev/null 2>&1 &
    waited=0
    until grep -qx ready "$recording" 2>/dev/null; do
      if [ "$waited" -ge 120 ]; then
        echo "::error::the network recorder did not start" >&2
        cat "$recording.log" >&2 || true
        exit 1
      fi
      sleep 0.5
      waited=$((waited + 1))
    done
    echo "network recorder running (pid $(cat "$recording.pid")), writing $recording"
    ;;
  stop)
    if [ ! -f "$recording.pid" ]; then
      # The evaluation reports the missing recording.
      echo "::warning::no network recorder was started" >&2
      exit 0
    fi
    pid=$(cat "$recording.pid")
    as_root kill -INT "$pid" || true
    waited=0
    while as_root kill -0 "$pid" 2>/dev/null; do
      if [ "$waited" -ge 60 ]; then
        echo "::warning::the network recorder did not stop; killing it" >&2
        as_root kill -KILL "$pid" || true
        break
      fi
      sleep 0.5
      waited=$((waited + 1))
    done
    as_root chmod a+r "$recording" "$recording.log" || true
    # bpftrace reports problems (lost events, warnings) on stderr.
    if [ -s "$recording.log" ]; then
      echo "bpftrace messages:"
      cat "$recording.log"
    fi
    ;;
  *)
    echo "usage: $0 start|stop <recording>" >&2
    exit 2
    ;;
esac
