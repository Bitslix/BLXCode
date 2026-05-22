#!/usr/bin/env bash
# Fail when UI stylesheets contain hardcoded colors outside token definition blocks.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

fail=0

echo "Checking styles.css for hardcoded colors outside :root typography block…"
python3 << 'PY'
import re, sys
from pathlib import Path
lines = Path("styles.css").read_text().splitlines()
in_root = False
root_end = 0
for i, line in enumerate(lines):
    if line.strip() == ":root {":
        in_root = True
    elif in_root and line.strip() == "}":
        root_end = i + 1
        break

bad = []
pat = re.compile(r"(#[0-9a-fA-F]{3,8}|rgba?\([^)]+\))")
for i, line in enumerate(lines[root_end:], root_end + 1):
    if line.strip().startswith("--"):
        continue
    if "var(--" in line and pat.search(line) is None:
        continue
    for m in pat.finditer(line):
        if "var(" in line[:m.start()] and ")" not in line[:m.start()]:
            pass
        bad.append((i, m.group(), line.strip()[:120]))

if bad:
    print(f"FAIL: {len(bad)} hardcoded color(s) in styles.css")
    for i, color, ctx in bad[:40]:
        print(f"  L{i}: {color}  {ctx}")
    if len(bad) > 40:
        print(f"  … and {len(bad) - 40} more")
    sys.exit(1)
print("OK: styles.css")
PY
fail=$?

if [[ $fail -ne 0 ]]; then
  echo "styles.css lint failed (hardcoded colors remain — extend scripts/tokenize_styles_css.py)"
  exit 1
fi

echo "Checking src/**/*.css for var(..., #literal) fallbacks…"
if rg -n 'var\([^)]+,\s*#' src --glob '*.css' >/tmp/theme-lint-fallbacks.txt 2>/dev/null; then
  echo "WARN: literal fallbacks in var():"
  cat /tmp/theme-lint-fallbacks.txt
  exit 1
fi

echo "All theme token lint checks passed."
