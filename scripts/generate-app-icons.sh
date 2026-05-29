#!/usr/bin/env bash
# Regenerate src-tauri/icons/ from public/blxcode.png (Tauri bundle + platform packs).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/src-tauri"
exec cargo tauri icon "$ROOT/public/blxcode.png" -o icons
