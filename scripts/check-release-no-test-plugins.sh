#!/usr/bin/env bash
# Fails if a test plugin is part of a release build (master prompt, section 20).
#
# Usage: scripts/check-release-no-test-plugins.sh <release binary>
set -euo pipefail

binary="${1:?usage: $0 <release binary>}"
manifest="src-tauri/Cargo.toml"
pattern='tauri-plugin-wdio|wdio-webdriver|tauri_plugin_wdio'

# 1. Dependency graph of a default (release) build, all target platforms.
if cargo tree --manifest-path "$manifest" --edges normal --target all --prefix none | grep -E "$pattern"; then
  echo "::error::a test plugin is part of the release dependency graph"
  exit 1
fi

# 2. The compiled binary itself.
if [ ! -f "$binary" ]; then
  echo "::error::release binary not found: $binary"
  exit 1
fi
if grep -a -q -E "$pattern" "$binary"; then
  echo "::error::test plugin code found in the release binary $binary"
  exit 1
fi

echo "OK: no test plugins in the release build ($binary)"
