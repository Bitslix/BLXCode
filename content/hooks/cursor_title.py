#!/usr/bin/env python3
"""blxcode: Cursor CLI ``beforeSubmitPrompt`` hook (cross-platform).

Cursor's hook payload exposes ``.prompt`` for ``beforeSubmitPrompt``
along with ``conversation_id`` and other base fields. We use the prompt
to refresh the terminal tab title — same OSC-2 trick as the Claude /
Codex / Gemini equivalents.

Cursor expects hook scripts to either return JSON on stdout or exit 0
silently. We must NOT print to stdout (that would be parsed and could
override agent permission/decision), so the OSC sequence goes to
``/dev/tty`` (or ``CONOUT$`` on Windows) directly.
"""
from __future__ import annotations

import json
import os
import re
import sys


MAX_TOPIC_LEN = 48
PREFIX = "cursor"


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
