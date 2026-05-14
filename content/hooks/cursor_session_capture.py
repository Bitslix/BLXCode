#!/usr/bin/env python3
"""blxcode: Cursor CLI ``sessionStart`` hook (cross-platform).

Persist {terminal_key -> {agent: cursor, session_id, conversation_id, ...}}
so blxcode can later issue ``cursor-agent --resume <id>`` for the
exact terminal slot instead of letting the user pick from a list.

Required env (set by blxcode when spawning the PTY):
- BLX_TERMINAL_KEY   composite "<workspace_id>:<terminal_slot_id>"
- BLX_SESSIONS_PATH  absolute path to sessions.json

Cursor's ``sessionStart`` payload contains ``session_id`` and
``conversation_id`` (the chat ID accepted by ``--resume``). We prefer
``conversation_id`` for the resume key when present, with ``session_id``
as a fallback — Cursor's resume CLI takes the chat/conversation id.
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
            f"cursor SKIP missing_env terminal_key={bool(terminal_key)} sessions_path={bool(sessions_path)}",
        )
        return 0

    payload = _read_payload()
    # cursor-agent --resume takes the chat / conversation id; prefer it,
    # fall back to session_id when conversation_id is missing.
    resume_id = payload.get("conversation_id")
    if not isinstance(resume_id, str) or not resume_id.strip():
        resume_id = payload.get("session_id")
    if not isinstance(resume_id, str) or not resume_id.strip():
        _diag_log(
            sessions_path,
            f"cursor SKIP no_resume_id key={terminal_key} payload_keys={list(payload.keys())}",
        )
        return 0

    workspace_roots = payload.get("workspace_roots")
    cwd = ""
    if isinstance(workspace_roots, list) and workspace_roots:
        first = workspace_roots[0]
        if isinstance(first, str):
            cwd = first
    if not cwd:
        cwd = os.getcwd()

    _diag_log(
        sessions_path,
        f"cursor WRITE key={terminal_key} resume_id={resume_id.strip()}",
    )
    state = _load_existing(sessions_path)
    if "terminals" not in state or not isinstance(state["terminals"], dict):
        state["terminals"] = {}
    state["version"] = 1
    state["terminals"][terminal_key] = {
        "agent": "cursor",
        "session_id": resume_id.strip(),
        "cwd": cwd,
        "source": "startup",
        "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    _atomic_write_json(sessions_path, state)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
