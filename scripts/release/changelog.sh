# shellcheck shell=bash
# CHANGELOG [Unreleased] → [version] — source only.

release_changelog_finalize() {
  local version="$1"
  local changelog
  changelog="$(release_changelog_path)"
  local args=(python3 "$RELEASE_SCRIPT_DIR/changelog_finalize.py" "$changelog" "$version")
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    args+=(--dry-run)
  fi
  "${args[@]}"
}
