#!/usr/bin/env python3
"""blxcode: Gemini CLI BeforeAgent hook (cross-platform).

Mirrors ``claude_title.py`` for Google's Gemini CLI. Gemini fires
``BeforeAgent`` for each user turn and the payload exposes ``.prompt`` —
same shape Claude uses for ``UserPromptSubmit``. We extract the first
line, derive a short topic and emit an OSC-2 sequence to the controlling
terminal so blxcode's terminal cell updates its tab title.

stdout is left empty so Gemini does not splice anything into model
context. Errors are swallowed.
"""
from __future__ import annotations

import json
import os
import re
import sys


MAX_TOPIC_LEN = 48
PREFIX = "gemini"


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
