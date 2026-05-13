#!/usr/bin/env python3
"""blxcode: Codex hook to set terminal title from the user's prompt.

Codex's hook mechanism is less standardised than Claude Code's, so this
script accepts the prompt from several sources, in order of preference:

1. The first positional CLI argument.
2. The ``BLX_PROMPT`` environment variable.
3. A JSON payload on stdin with a top-level ``prompt`` / ``input`` /
   ``message`` / ``user_prompt`` field.
4. Raw stdin text (used as-is).

Writes an xterm OSC-2 title sequence to the controlling terminal.
"""
from __future__ import annotations

import json
import os
import re
import sys


MAX_TOPIC_LEN = 48
PREFIX = "codex"


def _from_stdin() -> str:
    try:
        raw = sys.stdin.read()
    except Exception:
        return ""
    if not raw.strip():
        return ""
    try:
        data = json.loads(raw)
    except Exception:
        return raw
    if isinstance(data, dict):
        for key in ("prompt", "input", "message", "user_prompt"):
            value = data.get(key)
            if isinstance(value, str) and value.strip():
                return value
    return raw


def _resolve_prompt() -> str:
    if len(sys.argv) > 1 and sys.argv[1].strip():
        return sys.argv[1]
    env = os.environ.get("BLX_PROMPT", "")
    if env.strip():
        return env
    return _from_stdin()


def _shorten(prompt: str) -> str:
    first = prompt.splitlines()[0] if prompt else ""
    first = re.sub(r"\s+", " ", first).strip()
    if len(first) > MAX_TOPIC_LEN:
        first = first[: MAX_TOPIC_LEN - 1].rstrip() + "…"
    return first


def _write_osc(title: str) -> None:
    seq = f"\x1b]2;{title}\x07"
    targets: list[str] = []
    if os.name == "nt":
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
    prompt = _resolve_prompt()
    topic = _shorten(prompt) if isinstance(prompt, str) else ""
    if not topic:
        return 0
    _write_osc(f"{PREFIX} · {topic}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
