#!/usr/bin/env python3
"""blxcode: Cursor CLI stop hook — increment unread when status is completed."""
from __future__ import annotations

import os
import sys

_HOOKS_DIR = os.path.dirname(os.path.abspath(__file__))
if _HOOKS_DIR not in sys.path:
    sys.path.insert(0, _HOOKS_DIR)

from blxcode_notify import _read_payload, record_notification, should_count  # noqa: E402


def main() -> int:
    payload = _read_payload()
    if not should_count(payload, require_completed=True):
        return 0
    return record_notification()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
