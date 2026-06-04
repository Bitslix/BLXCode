#!/usr/bin/env python3
"""blxcode: Claude Code status-line usage capture.

Claude pipes status-line JSON to this command. We persist the latest payload
for BLXCode's terminal usage dropdown, then optionally run the user's previous
status-line command with the same stdin and forward its stdout.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import shlex
import subprocess
import sys
import tempfile
import time


def _atomic_write_json(path: str, value: dict) -> None:
    directory = os.path.dirname(path) or "."
    os.makedirs(directory, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".usage-", suffix=".tmp", dir=directory)
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


def _safe_key(raw: str) -> str:
    key = raw.strip()
    if (
        not key
        or len(key) > 512
        or "/" in key
        or "\\" in key
        or any(ord(ch) < 32 for ch in key)
    ):
        return ""
    return key


def _write_snapshot(raw: str) -> None:
    usage_path = os.environ.get("BLX_USAGE_PATH", "").strip()
    terminal_key = _safe_key(os.environ.get("BLX_TERMINAL_KEY", ""))
    if not usage_path or not terminal_key or not raw.strip():
        return
    try:
        payload = json.loads(raw)
    except Exception:
        return
    if not isinstance(payload, dict):
        return
    state = _load_existing(usage_path)
    terminals = state.get("terminals")
    if not isinstance(terminals, dict):
        terminals = {}
    terminals[terminal_key] = {
        "agent": "claude",
        "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "payload": payload,
    }
    state["version"] = 1
    state["terminals"] = terminals
    _atomic_write_json(usage_path, state)


def _decode_next(encoded: str) -> str:
    if not encoded:
        return ""
    try:
        return base64.b64decode(encoded.encode("ascii")).decode("utf-8", "replace").strip()
    except Exception:
        return ""


def _run_next(command: str, raw: str) -> int:
    if not command:
        return 0
    try:
        proc = subprocess.run(
            command,
            input=raw,
            text=True,
            shell=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=5,
            check=False,
        )
    except Exception:
        return 0
    if proc.stdout:
        sys.stdout.write(proc.stdout)
        sys.stdout.flush()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--next-b64", default="")
    args, _ = parser.parse_known_args()
    try:
        raw = sys.stdin.read()
    except Exception:
        raw = ""
    _write_snapshot(raw)
    return _run_next(_decode_next(args.next_b64), raw)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
