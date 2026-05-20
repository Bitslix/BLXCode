#!/usr/bin/env python3
"""Finalize CHANGELOG.md: move [Unreleased] to [version] - date and insert empty [Unreleased]."""
from __future__ import annotations

import argparse
import re
import sys
from datetime import date
from pathlib import Path

UNRELEASED_HEADER = "## [Unreleased]"
NEW_UNRELEASED = """## [Unreleased]

### Added

### Changed

### Fixed

### Removed

"""


def finalize(path: Path, version: str, release_date: str, dry_run: bool) -> None:
    text = path.read_text(encoding="utf-8")
    if UNRELEASED_HEADER not in text:
        raise SystemExit(f"{path}: missing {UNRELEASED_HEADER!r}")

    idx = text.index(UNRELEASED_HEADER)
    after = idx + len(UNRELEASED_HEADER)
    rest = text[after:]
    next_m = re.search(r"\n## \[", rest)
    if next_m is None:
        body = rest
        tail = ""
    else:
        body = rest[: next_m.start()]
        tail = rest[next_m.start() :]

    if not body.strip():
        print(f"warn: [Unreleased] section is empty", file=sys.stderr)

    versioned = f"## [{version}] - {release_date}"
    new_text = (
        text[:idx]
        + NEW_UNRELEASED
        + "\n"
        + versioned
        + body
        + tail
    )

    if dry_run:
        print(f"--- {path} (dry-run preview, first 40 lines of result) ---")
        for i, line in enumerate(new_text.splitlines()[:40]):
            print(line)
        if len(new_text.splitlines()) > 40:
            print("...")
        return

    path.write_text(new_text, encoding="utf-8")
    print(f"updated {path}")


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("changelog", type=Path)
    p.add_argument("version")
    p.add_argument("--date", default=date.today().isoformat())
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()
    finalize(args.changelog, args.version, args.date, args.dry_run)


if __name__ == "__main__":
    main()
