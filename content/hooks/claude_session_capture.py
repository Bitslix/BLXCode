#!/usr/bin/env python3
"""blxcode: Claude Code SessionStart hook (cross-platform).

Fires when Claude opens (or resumes) a session. We read the session_id
from the hook payload and persist a mapping keyed by the terminal slot
blxcode spawned, so on next launch blxcode can issue
`claude --resume <id>` for that exact slot.

Required env (set by blxcode when spawning the PTY):
- BLX_TERMINAL_KEY   composite "<workspace_id>:<terminal_slot_id>"
- BLX_SESSIONS_PATH  absolute path to sessions.json

stdout is left empty so Claude does not interpret the hook output.
Errors are swallowed — never block the user.
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


def _diag_log(sessions_path: str, line: str) -> None:
    """Append a single diagnostic line next to sessions.json. Used to
    debug why a SessionStart hook isn't producing a mapping (env missing,
    payload missing session_id, permission errors, etc.).
    """
    try:
        log_path = os.path.join(os.path.dirname(sessions_path) or ".", "sessions.log")
        os.makedirs(os.path.dirname(log_path) or ".", exist_ok=True)
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(
                time.strftime("%Y-%m-%dT%H:%M:%SZ ", time.gmtime()) + line + "\n"
            )
    except OSError:
        pass


def main() -> int:
    terminal_key = os.environ.get("BLX_TERMINAL_KEY", "").strip()
    sessions_path = os.environ.get("BLX_SESSIONS_PATH", "").strip()
    diag_path = sessions_path or os.path.expanduser("~/.cache/blxcode-sessions.log")
    if not terminal_key or not sessions_path:
        _diag_log(
            diag_path,
            f"claude SKIP missing_env terminal_key={bool(terminal_key)} sessions_path={bool(sessions_path)}",
        )
        return 0

    payload = _read_payload()
    session_id = payload.get("session_id")
    if not isinstance(session_id, str) or not session_id.strip():
        _diag_log(
            sessions_path,
            f"claude SKIP no_session_id key={terminal_key} payload_keys={list(payload.keys())}",
        )
        return 0

    cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) else os.getcwd()
    source = payload.get("source") if isinstance(payload.get("source"), str) else "startup"

    _diag_log(
        sessions_path,
        f"claude WRITE key={terminal_key} session_id={session_id.strip()} source={source}",
    )
    state = _load_existing(sessions_path)
    if "terminals" not in state or not isinstance(state["terminals"], dict):
        state["terminals"] = {}
    state["version"] = 1
    state["terminals"][terminal_key] = {
        "agent": "claude",
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
