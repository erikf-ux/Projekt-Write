#!/usr/bin/env bash
# Installs Ubuntu packages on a CI runner, robust against a broken package
# index (CI run 126: `apt-get update` fetched only packages.microsoft.com and
# the Ubuntu archive was missing, so no WebKitGTK package was found).
#
# Usage: bash scripts/apt-install.sh <package>...
#
# Each attempt refreshes the index and checks that the first package is known
# before installing. If the Ubuntu archive is still missing after the
# retries, the standard Ubuntu sources of the runner's release are restored.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <package>..." >&2
  exit 2
fi

probe="$1"

known() {
  apt-cache policy "$probe" 2>/dev/null | grep -q 'Candidate: [^(]'
}

refresh() {
  sudo apt-get update -o Acquire::Retries=3 || true
}

restore_sources() {
  # Ubuntu 24.04 keeps its sources in deb822 form; the release name comes
  # from the runner itself, nothing is hard-coded to one release.
  . /etc/os-release
  echo "::warning::Ubuntu archive missing from the package index; restoring the standard sources for ${VERSION_CODENAME}"
  sudo tee /etc/apt/sources.list.d/ubuntu.sources >/dev/null <<EOF
Types: deb
URIs: http://archive.ubuntu.com/ubuntu/
Suites: ${VERSION_CODENAME} ${VERSION_CODENAME}-updates ${VERSION_CODENAME}-backports
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

Types: deb
URIs: http://security.ubuntu.com/ubuntu/
Suites: ${VERSION_CODENAME}-security
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
EOF
}

for attempt in 1 2 3; do
  refresh
  if known; then
    break
  fi
  echo "Package index without ${probe} (attempt ${attempt}); retrying"
  if [ "$attempt" -eq 2 ]; then
    restore_sources
  fi
  sleep $((attempt * 10))
done

if ! known; then
  refresh
fi
if ! known; then
  echo "::error::${probe} is not in the package index; apt sources:"
  grep -rh '^[^#]' /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null || true
  exit 1
fi

for attempt in 1 2 3; do
  if sudo apt-get install -y -o Acquire::Retries=3 "$@"; then
    exit 0
  fi
  echo "apt-get install failed (attempt ${attempt}); refreshing and retrying"
  sleep $((attempt * 10))
  refresh
done
echo "::error::apt-get install failed three times"
exit 1
