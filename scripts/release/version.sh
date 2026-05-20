# shellcheck shell=bash
# Semver bump and sync version files — source only.

release_bump_version() {
  local part="$1"
  local current new
  current="$(release_read_version)"

  new="$(python3 - "$current" "$part" <<'PY'
import sys

def bump(v: str, part: str) -> str:
    m = v.split(".")
    if len(m) != 3 or not all(x.isdigit() for x in m):
        raise SystemExit(f"invalid semver: {v!r}")
    major, minor, patch = (int(x) for x in m)
    if part == "patch":
        patch += 1
    elif part == "minor":
        minor += 1
        patch = 0
    elif part == "major":
        major += 1
        minor = 0
        patch = 0
    else:
        raise SystemExit(f"unknown bump part: {part!r}")
    return f"{major}.{minor}.{patch}"

print(bump(sys.argv[1], sys.argv[2]))
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
