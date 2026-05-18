#!/usr/bin/env python3
"""blxcode: shared agent-completion notifier (cross-platform).

Increments an unread counter in `notifications.json` for the terminal
slot identified by `BLX_TERMINAL_KEY`. Called from agent-specific Stop/stop
wrapper scripts after validating the hook payload.

Required env (set by blxcode when spawning the PTY):
    BLX_TERMINAL_KEY       composite "<workspace_id>:<slot_id>:<pane_id>"
    BLX_NOTIFICATIONS_PATH absolute path to notifications.json
Optional:
    BLX_AGENT_SLUG         agent slug for accent metadata

Never prints to stdout — hooks must not inject into agent context.
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
    fd, tmp = tempfile.mkstemp(prefix=".notifications-", suffix=".tmp", dir=directory)
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


def should_count(payload: dict, *, require_completed: bool) -> bool:
    """Return False when the hook signals a blocked stop or non-success."""
    if payload.get("decision") == "block":
        return False
    hook_out = payload.get("hookSpecificOutput")
    if isinstance(hook_out, dict) and hook_out.get("permissionDecision") == "deny":
        return False
    if require_completed:
        status = payload.get("status")
        if isinstance(status, str) and status.strip().lower() != "completed":
            return False
    return True


def record_notification() -> int:
    terminal_key = os.environ.get("BLX_TERMINAL_KEY", "").strip()
    notifications_path = os.environ.get("BLX_NOTIFICATIONS_PATH", "").strip()
    if not terminal_key or not notifications_path:
        return 0

    agent = os.environ.get("BLX_AGENT_SLUG", "").strip()
    state = _load_existing(notifications_path)
    if "terminals" not in state or not isinstance(state["terminals"], dict):
        state["terminals"] = {}
    state["version"] = 1
    entry = state["terminals"].get(terminal_key)
    if not isinstance(entry, dict):
        entry = {}
    unread = entry.get("unread", 0)
    if not isinstance(unread, int):
        try:
            unread = int(unread)
        except (TypeError, ValueError):
            unread = 0
    entry["unread"] = unread + 1
    if agent:
        entry["agent"] = agent
    entry["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    state["terminals"][terminal_key] = entry
    _atomic_write_json(notifications_path, state)
    return 0


def main() -> int:
    return record_notification()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
