# shellcheck shell=bash
# Pre-build deps and cargo tauri build — source only.

release_prepare_deps() {
  release_require_cmd npm
  release_require_cmd cargo
  release_require_cmd rustup

  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: rustup target add wasm32-unknown-unknown"
    release_info "Would: npm ci --prefix frontend-js && npm run build:graph3d"
    return 0
  fi

  rustup target add wasm32-unknown-unknown

  if [[ -f "$RELEASE_ROOT/frontend-js/package-lock.json" ]]; then
    npm ci --prefix "$RELEASE_ROOT/frontend-js"
  else
    npm install --prefix "$RELEASE_ROOT/frontend-js"
  fi
  npm --prefix "$RELEASE_ROOT/frontend-js" run build:graph3d
}

release_build() {
  local platform
  platform="$(release_detect_platform)"
  local -a args=()

  case "$platform" in
    linux)
      export NO_STRIP=1
      export APPIMAGE_EXTRACT_AND_RUN=1
      release_info "Linux: NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 (AppImage strip workaround)"
      ;;
    macos)
      rustup target add aarch64-apple-darwin x86_64-apple-darwin
      args+=(--target universal-apple-darwin)
      ;;
    windows) ;;
    *)
      release_die "Unknown platform: $platform"
      ;;
  esac

  if [[ -n "${RELEASE_BUNDLES:-}" ]]; then
    # shellcheck disable=SC2206
    args+=(--bundles ${RELEASE_BUNDLES})
  fi

  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: cargo tauri build ${args[*]}"
    return 0
  fi

  release_check_signing
  (cd "$RELEASE_ROOT/src-tauri" && cargo tauri build "${args[@]}")
}

release_collect_artifact_paths() {
  local version="${1:-}"
  local -a found=()
  local root="$RELEASE_ROOT"
  while IFS= read -r -d '' f; do
    if [[ -n "$version" ]]; then
      local base="${f##*/}"
      if [[ "$base" != *"${version}"* ]]; then
        continue
      fi
    fi
    found+=("$f")
  done < <(
    find "$root/target" -path '*/release/bundle/*' \( \
      -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o \
      -name '*.dmg' -o -name '*.msi' -o -name '*.exe' -o \
      -name '*.sig' -o -name '*.app.tar.gz' \
    \) -type f -print0 2>/dev/null || true
  )
  if [[ ${#found[@]} -eq 0 ]]; then
    return 1
  fi
  printf '%s\n' "${found[@]}"
}

release_list_artifacts() {
  local version="${1:-$RELEASE_VERSION}"
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    local size
    size="$(du -h "$path" | awk '{print $1}')"
    release_info "  $path ($size)"
  done < <(release_collect_artifact_paths "$version" || true)
}
