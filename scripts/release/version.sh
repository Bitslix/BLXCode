# shellcheck shell=bash
# Semver bump and sync version files — source only.

release_bump_version() {
  local part="${1:-}"
  local current new
  current="$(release_read_version)"

  new="$(python3 - "$current" "$part" "${RELEASE_PRE_RELEASE:-0}" <<'PY'
import sys, re

def parse(v: str):
    mo = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:-pre\.(\d+))?", v)
    if not mo:
        raise SystemExit(f"invalid semver: {v!r}")
    major, minor, patch = (int(mo.group(i)) for i in range(1, 4))
    pre = int(mo.group(4)) if mo.group(4) else None
    return major, minor, patch, pre

def bump(v: str, spec: str, pre_release: bool) -> str:
    major, minor, patch, pre = parse(v)
    if pre_release and not spec:
        if pre is not None:
            return f"{major}.{minor}.{patch}-pre.{pre + 1}"
        return f"{major}.{minor}.{patch + 1}-pre.1"

    if not spec:
        raise SystemExit("missing bump spec: use patch|minor|major[+N] or --pre-release")
    # Accept "patch", "minor", "major" (+1) or "patch+N", "minor+N", "major+N" (+N).
    mo = re.fullmatch(r"(patch|minor|major)(?:\+(\d+))?", spec)
    if not mo:
        raise SystemExit(f"unknown bump spec: {spec!r} (use patch|minor|major[+N])")
    part = mo.group(1)
    step = int(mo.group(2) or "1")
    if step < 1:
        raise SystemExit(f"bump step must be >= 1, got {step}")

    if part == "patch":
        patch += step
    elif part == "minor":
        minor += step
        patch = 0
    elif part == "major":
        major += step
        minor = 0
        patch = 0
    if pre_release:
        return f"{major}.{minor}.{patch}-pre.1"
    return f"{major}.{minor}.{patch}"

print(bump(sys.argv[1], sys.argv[2], sys.argv[3] == "1"))
PY
)"

  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would bump $current -> $new ($part)"
    RELEASE_VERSION="$new"
    if [[ "${RELEASE_NO_CHANGELOG:-0}" != "1" ]]; then
      release_changelog_finalize "$new"
    fi
    return 0
  fi

  python3 - "$RELEASE_ROOT" "$new" <<'PY'
import json, re, sys
root, new = sys.argv[1], sys.argv[2]

conf = f"{root}/src-tauri/tauri.conf.json"
with open(conf, encoding="utf-8") as f:
    data = json.load(f)
data["version"] = new
with open(conf, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")

for rel in ("Cargo.toml", "src-tauri/Cargo.toml"):
    path = f"{root}/{rel}"
    text = open(path, encoding="utf-8").read()
    text2, n = re.subn(
        r'^(version\s*=\s*")[^"]+(")',
        rf'\g<1>{new}\2',
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if n != 1:
        raise SystemExit(f"{path}: could not update version = ...")
    open(path, "w", encoding="utf-8").write(text2)

for rel in ("package.json", "package-lock.json"):
    path = f"{root}/{rel}"
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    data["version"] = new
    if rel == "package-lock.json":
        data.setdefault("packages", {}).setdefault("", {})["version"] = new
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
PY

  release_info "Bumped $current -> $new ($part)"
  RELEASE_VERSION="$new"

  if [[ "${RELEASE_NO_CHANGELOG:-0}" != "1" ]]; then
    release_changelog_finalize "$new"
  fi
}

release_check_bump_conflict() {
  local version="$1"
  local tag="v${version}"
  if release_remote_tag_exists "$tag" || release_gh_release_exists "$tag"; then
    release_die "Release $tag already exists on GitHub; refusing --bump to $version"
  fi
}
