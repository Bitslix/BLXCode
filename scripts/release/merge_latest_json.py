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


# Tauri v2's `bundle.createUpdaterArtifacts` setting determines which Windows files exist
# and which one belongs in latest.json's `url`:
#   - `true` (current project setting): only the raw installers exist. The updater downloads
#     and runs them directly. latest.json -> *-setup.exe (NSIS) or *.msi (WiX).
#   - `"v1Compatible"` (legacy/migration): both raw installers AND zip-wrapped variants are
#     emitted. The .zip is the "updater bundle"; v1 clients require the .zip URL.
# We accept all four suffixes and pick by priority: if the zip-wrapped form is present (i.e.
# v1Compatible build), prefer it; otherwise fall back to the direct installer signature.
# Either choice produces a valid latest.json for its respective mode.
WINDOWS_SUFFIX_PRIORITY: tuple[str, ...] = (
    ".nsis.zip.sig",
    ".msi.zip.sig",
    ".exe.sig",
    ".msi.sig",
)


def windows_suffix_rank(name: str) -> int:
    name = name.lower()
    for index, suffix in enumerate(WINDOWS_SUFFIX_PRIORITY):
        if name.endswith(suffix):
            return index
    return len(WINDOWS_SUFFIX_PRIORITY)


def infer_platform(path: Path) -> str | None:
    name = path.name.lower()
    full = str(path).lower()
    if name.endswith(".appimage.sig"):
        arch = "aarch64" if any(token in full for token in ("aarch64", "arm64")) else "x86_64"
        return f"linux-{arch}"
    if name.endswith(".app.tar.gz.sig"):
        return "darwin-universal"
    if name.endswith(WINDOWS_SUFFIX_PRIORITY):
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


def _sort_key(platform: str, sig: Path) -> tuple[int, str]:
    name = sig.name.lower()
    if platform.startswith("windows-"):
        return (windows_suffix_rank(name), name)
    return (0, name)


def build_manifest(repo: str, tag: str, version: str, notes: str, artifacts: list[Path]) -> dict:
    manifest = load_existing(repo, tag)
    manifest["version"] = version
    manifest.setdefault("notes", notes)
    manifest.setdefault("pub_date", datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"))
    platforms = manifest.setdefault("platforms", {})

    # Group signatures by platform first, then for each platform pick the highest-priority
    # signature whose payload actually exists on disk. This lets us prefer the .nsis.zip /
    # .msi.zip variants (what the Tauri v2 updater fetches) over the raw .exe / .msi.
    grouped: dict[str, list[Path]] = {}
    for sig in artifacts:
        platform = infer_platform(sig)
        if platform is None:
            continue
        grouped.setdefault(platform, []).append(sig)

    for platform, sigs in grouped.items():
        sigs = sorted(sigs, key=lambda s: _sort_key(platform, s))
        chosen: tuple[Path, Path, str] | None = None
        for sig in sigs:
            payload = payload_for_sig(sig)
            if not payload.exists():
                continue
            signature = sig.read_text(encoding="utf-8").strip()
            if not signature:
                continue
            chosen = (sig, payload, signature)
            break
        if chosen is None:
            raise SystemExit(
                f"no usable signature/payload pair found for platform {platform}: "
                + ", ".join(str(s) for s in sigs)
            )
        _, payload, signature = chosen
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
