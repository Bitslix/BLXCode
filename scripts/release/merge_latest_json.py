#!/usr/bin/env python3
"""Merge Tauri updater artifacts into one GitHub Releases latest.json."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path


def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, check=check, text=True, capture_output=True)


def infer_platform(path: Path) -> str | None:
    name = path.name.lower()
    full = str(path).lower()
    if name.endswith(".appimage.sig"):
        arch = "aarch64" if any(token in full for token in ("aarch64", "arm64")) else "x86_64"
        return f"linux-{arch}"
    if name.endswith(".app.tar.gz.sig"):
        return "darwin-universal"
    if name.endswith(".exe.sig") or name.endswith(".msi.sig"):
        return "windows-x86_64"
    return None


def payload_for_sig(sig_path: Path) -> Path:
    return sig_path.with_name(sig_path.name[:-4])


def load_existing(repo: str, tag: str) -> dict:
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        result = run(
            ["gh", "release", "download", tag, "-R", repo, "-p", "latest.json", "-D", str(tmp)],
            check=False,
        )
        latest = tmp / "latest.json"
        if result.returncode != 0 or not latest.exists():
            return {}
        return json.loads(latest.read_text(encoding="utf-8"))


def release_asset_names(repo: str, tag: str) -> set[str]:
    result = run(["gh", "release", "view", tag, "-R", repo, "--json", "assets", "-q", ".assets[].name"])
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def build_manifest(repo: str, tag: str, version: str, notes: str, artifacts: list[Path]) -> dict:
    manifest = load_existing(repo, tag)
    manifest["version"] = version
    manifest.setdefault("notes", notes)
    manifest.setdefault("pub_date", datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"))
    platforms = manifest.setdefault("platforms", {})

    for sig_path in sorted(artifacts, key=lambda p: p.name.lower()):
        platform = infer_platform(sig_path)
        if platform is None:
            continue
        if (
            platform.startswith("windows-")
            and sig_path.name.lower().endswith(".msi.sig")
            and platform in platforms
            and str(platforms[platform].get("url", "")).lower().endswith(".exe")
        ):
            continue
        payload = payload_for_sig(sig_path)
        if not payload.exists():
            raise SystemExit(f"signature has no matching payload: {sig_path}")
        signature = sig_path.read_text(encoding="utf-8").strip()
        if not signature:
            raise SystemExit(f"empty signature: {sig_path}")
        platforms[platform] = {
            "signature": signature,
            "url": f"https://github.com/{repo}/releases/download/{tag}/{payload.name}",
        }

    if not platforms:
        raise SystemExit("no updater signatures found; build with TAURI_SIGNING_PRIVATE_KEY first")
    return manifest


def validate_assets(repo: str, tag: str, manifest: dict) -> None:
    assets = release_asset_names(repo, tag)
    missing: list[str] = []
    for entry in manifest.get("platforms", {}).values():
        name = str(entry.get("url", "")).rsplit("/", 1)[-1]
        if name not in assets:
            missing.append(name)
    if missing:
        raise SystemExit("latest.json references missing release assets: " + ", ".join(sorted(missing)))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--notes", default="See CHANGELOG.md in the repository.")
    parser.add_argument("--output", default="latest.json")
    parser.add_argument("artifacts", nargs="+")
    args = parser.parse_args()

    manifest = build_manifest(
        args.repo,
        args.tag,
        args.version,
        args.notes,
        [Path(path) for path in args.artifacts],
    )
    validate_assets(args.repo, args.tag, manifest)
    Path(args.output).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
