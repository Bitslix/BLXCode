# shellcheck shell=bash
# GitHub release upload — source only.

release_gh_release_exists() {
  local tag="$1"
  command -v gh >/dev/null 2>&1 || return 1
  gh release view "$tag" -R "$RELEASE_GH_REPO" --json tagName >/dev/null 2>&1
}

release_gh_list_asset_names() {
  local tag="$1"
  gh release view "$tag" -R "$RELEASE_GH_REPO" --json assets -q '.assets[].name' 2>/dev/null || true
}

release_gh_create_release() {
  local tag="$1"
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: gh release create $tag -R $RELEASE_GH_REPO --draft --title BLXCode $tag"
    return 0
  fi
  gh release create "$tag" -R "$RELEASE_GH_REPO" \
    --draft \
    --title "BLXCode $tag" \
    --notes "See CHANGELOG.md in the repository."
}

release_filter_upload_files() {
  local tag="$1"
  shift
  local -a inputs=("$@")
  local -a out=()
  local existing name base

  if [[ "${RELEASE_CLOBBER:-0}" == "1" ]]; then
    printf '%s\n' "${inputs[@]}"
    return 0
  fi

  mapfile -t existing < <(release_gh_list_asset_names "$tag")
  for f in "${inputs[@]}"; do
    base="$(basename "$f")"
    local skip=0
    for e in "${existing[@]}"; do
      if [[ "$e" == "$base" ]]; then
        release_warn "Skipping existing asset: $base (use --clobber to replace)"
        skip=1
        break
      fi
    done
    if [[ "$skip" == "0" ]]; then
      out+=("$f")
    fi
  done
  if [[ ${#out[@]} -eq 0 ]]; then
    return 1
  fi
  printf '%s\n' "${out[@]}"
}

release_upload_artifacts() {
  local version="$1"
  local tag="v${version}"

  release_require_cmd gh

  local -a artifacts=()
  mapfile -t artifacts < <(release_collect_artifact_paths "$version" || true)
  if [[ ${#artifacts[@]} -eq 0 ]]; then
    release_die "No bundle artifacts under target/**/release/bundle/. Run --build first."
  fi

  local release_exists=0
  if release_gh_release_exists "$tag"; then
    release_exists=1
    release_info "Using existing GitHub release $tag"
  elif release_remote_tag_exists "$tag" || release_local_tag_exists "$tag"; then
    release_info "Tag $tag exists; creating draft release"
    release_gh_create_release "$tag"
    release_exists=1
  else
    release_info "Creating new draft release $tag"
    release_gh_create_release "$tag"
    release_exists=1
  fi

  local -a to_upload=()
  if [[ "$release_exists" == "1" ]] && [[ "${RELEASE_CLOBBER:-0}" != "1" ]]; then
    mapfile -t to_upload < <(release_filter_upload_files "$tag" "${artifacts[@]}" || true)
  else
    to_upload=("${artifacts[@]}")
  fi

  if [[ ${#to_upload[@]} -eq 0 ]]; then
    release_warn "Nothing to upload (all assets already on release?)"
    return 0
  fi

  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would upload to $tag:"
    local f
    for f in "${to_upload[@]}"; do
      release_info "  $f"
    done
    return 0
  fi

  local -a gh_args=(gh release upload "$tag" -R "$RELEASE_GH_REPO")
  if [[ "${RELEASE_CLOBBER:-0}" == "1" ]]; then
    gh_args+=(--clobber)
  fi
  "${gh_args[@]}" "${to_upload[@]}"
  release_info "Uploaded ${#to_upload[@]} file(s) to $tag on $RELEASE_GH_REPO"
  release_upload_latest_json "$tag" "$version" "${artifacts[@]}"
}

release_upload_latest_json() {
  local tag="$1"
  local version="$2"
  shift 2
  local -a artifacts=("$@")
  local -a signatures=()
  local f

  for f in "${artifacts[@]}"; do
    if [[ "$f" == *.sig ]]; then
      signatures+=("$f")
    fi
  done

  if [[ ${#signatures[@]} -eq 0 ]]; then
    release_warn "No updater signatures found; skipping latest.json upload"
    return 0
  fi

  release_require_cmd python3
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: merge and upload latest.json for $tag"
    return 0
  fi

  local latest_path="$RELEASE_ROOT/target/latest.json"
  python3 "$RELEASE_ROOT/scripts/release/merge_latest_json.py" \
    --repo "$RELEASE_GH_REPO" \
    --tag "$tag" \
    --version "$version" \
    --output "$latest_path" \
    "${signatures[@]}"

  gh release upload "$tag" -R "$RELEASE_GH_REPO" --clobber "$latest_path"
  release_info "Uploaded canonical latest.json to $tag"
}
