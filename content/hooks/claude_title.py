#!/usr/bin/env python3
"""blxcode: Claude Code UserPromptSubmit hook (cross-platform).

Reads the hook JSON payload on stdin, derives a short topic from
``.prompt`` and writes an xterm OSC-2 window-title sequence directly to
the controlling terminal so the embedding terminal updates its title.

stdout is left empty so Claude does not inject anything into the model
context. Errors are swallowed -- a hook must never block the user.
"""
from __future__ import annotations

import json
import os
import re
import sys


MAX_TOPIC_LEN = 48
PREFIX = "claude"


def _read_payload() -> dict:
    try:
        raw = sys.stdin.read()
    except Exception:
        return {}
    if not raw.strip():
        return {}
    try:
        data = json.loads(raw)
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _shorten(prompt: str) -> str:
    first = prompt.splitlines()[0] if prompt else ""
    first = re.sub(r"\s+", " ", first).strip()
    if len(first) > MAX_TOPIC_LEN:
        first = first[: MAX_TOPIC_LEN - 1].rstrip() + "…"
    return first


def _write_osc(title: str) -> None:
    # OSC 2 ; <title> BEL — recognised by xterm, Windows Terminal,
    # ConEmu, kitty, alacritty, iTerm2 and blxcode's xterm.js cell.
    seq = f"\x1b]2;{title}\x07"
    targets: list[str] = []
    if os.name == "nt":
        # CONOUT$ is the real console output handle even when stdout is
        # redirected (Claude captures stdout for context injection).
        targets.append("CONOUT$")
        targets.append("CON")
    else:
        targets.append("/dev/tty")
    for path in targets:
        try:
            with open(path, "w", encoding="utf-8", errors="replace") as fh:
                fh.write(seq)
                fh.flush()
            return
        except OSError:
            continue


def main() -> int:
    data = _read_payload()
    prompt = data.get("prompt")
    if not isinstance(prompt, str):
        return 0
    topic = _shorten(prompt)
    if not topic:
        return 0
    _write_osc(f"{PREFIX} · {topic}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
