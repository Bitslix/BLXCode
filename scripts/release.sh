#!/usr/bin/env bash
# BLXCode release: optional semver bump, signed tauri build, GitHub release upload.
set -euo pipefail

RELEASE_EXIT_USER=1
RELEASE_EXIT_BUILD=2

RELEASE_DRY_RUN=0
RELEASE_REQUIRE_SIGNING=0
RELEASE_NO_CHANGELOG=0
RELEASE_CLOBBER=0
RELEASE_DO_BUILD=0
RELEASE_DO_UPLOAD=0
RELEASE_UPLOAD_ONLY=0
RELEASE_DO_TAG=0
RELEASE_DO_PUSH=0
RELEASE_DO_COMMIT=0
RELEASE_BUMP=""
RELEASE_BUNDLES=""
RELEASE_PLATFORM_OVERRIDE=""
RELEASE_LINUX_ARCH="${RELEASE_LINUX_ARCH:-all}"
RELEASE_VERSION=""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=release/common.sh
source "$SCRIPT_DIR/release/common.sh"
# shellcheck source=release/changelog.sh
source "$SCRIPT_DIR/release/changelog.sh"
# shellcheck source=release/version.sh
source "$SCRIPT_DIR/release/version.sh"
# shellcheck source=release/build.sh
source "$SCRIPT_DIR/release/build.sh"
# shellcheck source=release/upload.sh
source "$SCRIPT_DIR/release/upload.sh"

release_usage() {
  cat <<'EOF'
Usage: ./scripts/release.sh [options]

Build signed Tauri bundles for the current host (Linux sets NO_STRIP for AppImage).
Optionally bump version, rewrite CHANGELOG, tag, push, and upload to GitHub.

Options:
  --bump patch|minor|major   Bump version in Cargo.toml + tauri.conf.json + CHANGELOG
  --no-changelog             Skip CHANGELOG rewrite on bump
  --build                    Run cargo tauri build (default when not --upload-only / --no-build)
  --no-build                 Skip build
  --bundles LIST             Pass to cargo tauri build --bundles (e.g. appimage)
  --tag                      Create annotated git tag v{version}
  --push                     git push + push tag (requires --tag)
  --commit                   Commit version + CHANGELOG files
  --upload                   Upload bundle artifacts to GitHub release
  --upload-only              Only upload (no bump/build/tag)
  --clobber                  Replace existing release assets with same name
  --require-signing          Require TAURI_SIGNING_PRIVATE_KEY (default: unsigned OK)
  --allow-unsigned           Alias for default unsigned build (no-op)
  --platform linux|macos|windows
  --linux-arch native|amd64|arm64|all
                             Linux only: deb/rpm/AppImage per arch (default: all)
  --dry-run                  Print planned actions only
  -h, --help                 Show this help

Examples:
  ./scripts/release.sh
  ./scripts/release.sh --bump patch --build --upload
  ./scripts/release.sh --bump patch --tag --push --no-build
  ./scripts/release.sh --build --upload
  ./scripts/release.sh --upload-only
  ./scripts/release.sh --linux-arch all   # unsigned OK by default
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bump)
      RELEASE_BUMP="${2:?--bump requires patch|minor|major}"
      shift 2
      ;;
    --no-changelog) RELEASE_NO_CHANGELOG=1; shift ;;
    --build) RELEASE_DO_BUILD=1; shift ;;
    --no-build) RELEASE_DO_BUILD=0; RELEASE_NO_BUILD=1; shift ;;
    --bundles)
      RELEASE_BUNDLES="${2:?--bundles requires a list}"
      shift 2
      ;;
    --tag) RELEASE_DO_TAG=1; shift ;;
    --push) RELEASE_DO_PUSH=1; shift ;;
    --commit) RELEASE_DO_COMMIT=1; shift ;;
    --upload) RELEASE_DO_UPLOAD=1; shift ;;
    --upload-only)
      RELEASE_UPLOAD_ONLY=1
      RELEASE_DO_UPLOAD=1
      shift
      ;;
    --clobber) RELEASE_CLOBBER=1; shift ;;
    --require-signing) RELEASE_REQUIRE_SIGNING=1; shift ;;
    --allow-unsigned) shift ;;
    --platform)
      RELEASE_PLATFORM_OVERRIDE="${2:?--platform requires linux|macos|windows}"
      shift 2
      ;;
    --linux-arch)
      RELEASE_LINUX_ARCH="${2:?--linux-arch requires native|amd64|arm64|all}"
      shift 2
      ;;
    --dry-run) RELEASE_DRY_RUN=1; shift ;;
    -h|--help)
      release_usage
      exit 0
      ;;
    *)
      release_die "Unknown option: $1 (use --help)"
      ;;
  esac
done

release_common_init
release_info "Git remote: $RELEASE_GIT_REMOTE | GitHub: $RELEASE_GH_REPO"

# Default: build unless upload-only or explicit --no-build
if [[ "${RELEASE_UPLOAD_ONLY:-0}" == "1" ]]; then
  RELEASE_DO_BUILD=0
elif [[ "${RELEASE_NO_BUILD:-0}" != "1" ]] && [[ "$RELEASE_DO_BUILD" == "0" ]] && [[ "$RELEASE_DO_TAG" == "0" ]] && [[ -z "$RELEASE_BUMP" ]]; then
  RELEASE_DO_BUILD=1
elif [[ -n "$RELEASE_BUMP" ]] && [[ "${RELEASE_NO_BUILD:-0}" != "1" ]] && [[ "$RELEASE_DO_BUILD" == "0" ]] && [[ "${RELEASE_UPLOAD_ONLY:-0}" != "1" ]]; then
  RELEASE_DO_BUILD=1
fi

if [[ "$RELEASE_DO_PUSH" == "1" ]] && [[ "$RELEASE_DO_TAG" != "1" ]]; then
  release_die "--push requires --tag"
fi

RELEASE_VERSION="$(release_read_version)"
release_info "Project version: $RELEASE_VERSION (platform: $(release_detect_platform))"

release_load_env

tag_current="v${RELEASE_VERSION}"
existing_release=0
if release_gh_release_exists "$tag_current"; then
  existing_release=1
  release_info "GitHub release $tag_current already exists"
elif release_remote_tag_exists "$tag_current"; then
  existing_release=1
  release_info "Remote tag $tag_current already exists"
fi

if [[ "${RELEASE_UPLOAD_ONLY:-0}" == "1" ]]; then
  RELEASE_BUMP=""
  RELEASE_DO_TAG=0
  RELEASE_DO_BUILD=0
fi

if [[ -n "$RELEASE_BUMP" ]]; then
  release_bump_version "$RELEASE_BUMP"
  tag_current="v${RELEASE_VERSION}"
  if release_gh_release_exists "$tag_current" 2>/dev/null || release_remote_tag_exists "$tag_current"; then
    release_die "Release $tag_current already exists on GitHub after bump"
  fi
fi

if [[ "$RELEASE_DO_BUILD" == "1" ]]; then
  release_prepare_deps
  if ! release_build; then
    exit "$RELEASE_EXIT_BUILD"
  fi
  release_info "Build artifacts (v${RELEASE_VERSION}):"
  release_list_artifacts "$RELEASE_VERSION"
fi

if [[ "$RELEASE_DO_COMMIT" == "1" ]]; then
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: git commit version + CHANGELOG"
  else
    git add CHANGELOG.md Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json
    if [[ -n "$(git status --porcelain scripts/release.sh scripts/release/ .gitignore docs/user/building.md 2>/dev/null)" ]]; then
      git add scripts/release.sh scripts/release/ .gitignore docs/user/building.md
    fi
    git commit -m "chore: release v${RELEASE_VERSION}"
    release_info "Committed release files"
  fi
fi

if [[ "$RELEASE_DO_TAG" == "1" ]]; then
  release_git_dirty_warn
  if release_remote_tag_exists "$tag_current" || release_local_tag_exists "$tag_current"; then
    release_warn "Tag $tag_current already exists; skipping git tag (use --upload to add assets)"
  elif [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: git tag -a $tag_current -m \"BLXCode $RELEASE_VERSION\""
  else
    git tag -a "$tag_current" -m "BLXCode $RELEASE_VERSION"
    release_info "Created tag $tag_current"
  fi
fi

if [[ "$RELEASE_DO_PUSH" == "1" ]]; then
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: git push $RELEASE_GIT_REMOTE && git push $RELEASE_GIT_REMOTE $tag_current"
  else
    git push "$RELEASE_GIT_REMOTE"
    if release_local_tag_exists "$tag_current"; then
      if ! release_remote_tag_exists "$tag_current"; then
        git push "$RELEASE_GIT_REMOTE" "$tag_current"
        release_info "Pushed tag $tag_current to $RELEASE_GIT_REMOTE"
      else
        release_info "Tag $tag_current already on $RELEASE_GIT_REMOTE; skipping tag push"
      fi
    fi
  fi
fi

if [[ "$RELEASE_DO_UPLOAD" == "1" ]]; then
  release_upload_artifacts "$RELEASE_VERSION"
fi

release_info "Done."
