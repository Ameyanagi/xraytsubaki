#!/usr/bin/env bash
# Build a portable Apple Silicon/Intel app bundle on its native macOS host.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo "This packaging script requires macOS." >&2
  exit 1
fi
cargo build --release --locked -p rexafs-gui --no-default-features --features refeff-runner
python3 scripts/package-desktop.py
