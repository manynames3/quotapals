#!/usr/bin/env bash
# Backward-compatible entry point for existing QuotaPals installation instructions.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$HERE/install-tokenrunway-macos.sh" "$@"
