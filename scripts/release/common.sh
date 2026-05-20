# shellcheck shell=bash
# Shared helpers for scripts/release.sh — source only.

release_common_init() {
  set -euo pipefail
  RELEASE_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  RELEASE_ROOT="$(cd "$RELEASE_SCRIPT_DIR/../.." && pwd)"
  cd "$RELEASE_ROOT"
  RELEASE_GIT_REMOTE="$(release_git_remote)"
  RELEASE_GH_REPO="$(release_gh_repo)"
}

release_git_remote() {
  if [[ -n "${RELEASE_GIT_REMOTE_OVERRIDE:-}" ]]; then
    printf '%s' "$RELEASE_GIT_REMOTE_OVERRIDE"
    return
  fi
  local upstream
  upstream="$(git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null || true)"
  if [[ "$upstream" == */* ]]; then
    printf '%s' "${upstream%%/*}"
    return
  fi
  printf 'origin'
}

release_gh_repo() {
  if [[ -n "${RELEASE_GH_REPO_OVERRIDE:-}" ]]; then
    printf '%s' "$RELEASE_GH_REPO_OVERRIDE"
    return
  fi
  local remote url
  remote="$(release_git_remote)"
  url="$(git remote get-url "$remote" 2>/dev/null || true)"
  python3 - "$url" <<'PY'
import re, sys
url = sys.argv[1]
m = re.search(r"github\.com[:/]([^/]+)/([^/.]+)", url)
if not m:
    raise SystemExit(f"cannot parse GitHub repo from remote URL: {url!r}")
print(f"{m.group(1)}/{m.group(2)}")
PY
}

release_log() {
  local level="$1"
  shift
  printf '[release:%s] %s\n' "$level" "$*" >&2
}

release_info() { release_log info "$@"; }
release_warn() { release_log warn "$@"; }
release_err() { release_log error "$@"; }

release_die() {
  release_err "$@"
  exit "${RELEASE_EXIT_USER:-1}"
}

release_tauri_conf() {
  printf '%s/src-tauri/tauri.conf.json' "$RELEASE_ROOT"
}

release_changelog_path() {
  printf '%s/CHANGELOG.md' "$RELEASE_ROOT"
}

release_read_version() {
  python3 - "$RELEASE_ROOT" <<'PY'
import json, sys
path = sys.argv[1] + "/src-tauri/tauri.conf.json"
with open(path, encoding="utf-8") as f:
    print(json.load(f)["version"])
PY
}

release_detect_platform() {
  if [[ -n "${RELEASE_PLATFORM_OVERRIDE:-}" ]]; then
    printf '%s' "$RELEASE_PLATFORM_OVERRIDE"
    return
  fi
  case "$(uname -s)" in
    Linux) printf 'linux' ;;
    Darwin) printf 'macos' ;;
    MINGW*|MSYS*|CYGWIN*) printf 'windows' ;;
    *)
      release_die "Unsupported OS: $(uname -s)"
      ;;
  esac
}

release_load_env() {
  local env_file="$RELEASE_ROOT/.env.release"
  if [[ -f "$env_file" ]]; then
    release_info "Loading $env_file"
    # shellcheck disable=SC1090
    set -a
    source "$env_file"
    set +a
  fi
}

release_check_signing() {
  if [[ "${RELEASE_REQUIRE_SIGNING:-0}" == "1" ]]; then
    if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
      release_die "TAURI_SIGNING_PRIVATE_KEY is not set (--require-signing)"
    fi
    return 0
  fi
  if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
    release_warn "Unsigned build (no TAURI_SIGNING_PRIVATE_KEY; no Apple/Windows code signing required)"
  fi
}

release_git_dirty_warn() {
  if git diff --quiet && git diff --cached --quiet; then
    return 0
  fi
  release_warn "Working tree has uncommitted changes"
}

release_require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    release_die "Required command not found: $cmd"
  fi
}

release_remote_tag_exists() {
  local tag="$1"
  local remote="${RELEASE_GIT_REMOTE:-origin}"
  git ls-remote --exit-code --tags "$remote" "refs/tags/${tag}" >/dev/null 2>&1
}

release_local_tag_exists() {
  local tag="$1"
  git rev-parse "refs/tags/${tag}" >/dev/null 2>&1
}
