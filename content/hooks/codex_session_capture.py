#!/usr/bin/env python3
"""blxcode: Codex SessionStart hook (cross-platform).

Same idea as `claude_session_capture.py`: persist
{terminal_key -> {agent: codex, session_id, ...}} so blxcode can later
resume the precise Codex session for that terminal slot with
`codex resume <id>` instead of the cwd-wide `codex resume --last`.

Required env (set by blxcode when spawning the PTY):
- BLX_TERMINAL_KEY   composite "<workspace_id>:<terminal_slot_id>"
- BLX_SESSIONS_PATH  absolute path to sessions.json

Codex's SessionStart payload differs slightly from Claude's but the keys
we need (`session_id`, `cwd`, `source`) are present.
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
import time


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


def _atomic_write_json(path: str, value: dict) -> None:
    directory = os.path.dirname(path) or "."
    os.makedirs(directory, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".sessions-", suffix=".tmp", dir=directory)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(value, fh, indent=2)
            fh.flush()
            try:
                os.fsync(fh.fileno())
            except OSError:
                pass
        os.replace(tmp, path)
    except Exception:
        try:
            os.unlink(tmp)
        except OSError:
            pass


def _load_existing(path: str) -> dict:
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        if isinstance(data, dict):
            return data
    except (OSError, json.JSONDecodeError):
        pass
    return {"version": 1, "terminals": {}}


def main() -> int:
    terminal_key = os.environ.get("BLX_TERMINAL_KEY", "").strip()
    sessions_path = os.environ.get("BLX_SESSIONS_PATH", "").strip()
    if not terminal_key or not sessions_path:
        return 0

    payload = _read_payload()
    session_id = payload.get("session_id")
    if not isinstance(session_id, str) or not session_id.strip():
        return 0

    cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) else os.getcwd()
    source = payload.get("source") if isinstance(payload.get("source"), str) else "startup"

    state = _load_existing(sessions_path)
    if "terminals" not in state or not isinstance(state["terminals"], dict):
        state["terminals"] = {}
    state["version"] = 1
    state["terminals"][terminal_key] = {
        "agent": "codex",
        "session_id": session_id.strip(),
        "cwd": cwd,
        "source": source,
        "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    _atomic_write_json(sessions_path, state)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
