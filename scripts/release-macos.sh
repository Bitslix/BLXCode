#!/usr/bin/env bash
# macOS-focused launcher for the Bash release pipeline.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/release.sh" "$@" --platform macos
