# shellcheck shell=bash
# Linux multi-arch release builds — source only.

release_linux_cpu() {
  case "$(uname -m)" in
    x86_64 | amd64) printf 'amd64' ;;
    aarch64 | arm64) printf 'arm64' ;;
    *) printf '%s' "$(uname -m)" ;;
  esac
}

release_linux_cross_linker_ready() {
  local triple="$1"
  case "$triple" in
    aarch64-unknown-linux-gnu) command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 ;;
    x86_64-unknown-linux-gnu) command -v x86_64-linux-gnu-gcc >/dev/null 2>&1 ;;
    *) return 1 ;;
  esac
}

release_linux_cross_pkg_config_ready() {
  local triple="$1"
  local label="$2"
  local pkg_config_wrapper=""
  local pkg_config_libdir=""
  local -a required=(glib-2.0 gobject-2.0 gio-2.0 gtk+-3.0 webkit2gtk-4.1)

  case "$triple" in
    aarch64-unknown-linux-gnu)
      pkg_config_wrapper="aarch64-linux-gnu-pkg-config"
      pkg_config_libdir="/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
      ;;
    x86_64-unknown-linux-gnu)
      pkg_config_wrapper="x86_64-linux-gnu-pkg-config"
      pkg_config_libdir="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
      ;;
    *) return 1 ;;
  esac

  if command -v "$pkg_config_wrapper" >/dev/null 2>&1; then
    if "$pkg_config_wrapper" --exists "${required[@]}" >/dev/null 2>&1; then
      export PKG_CONFIG="$pkg_config_wrapper"
      export PKG_CONFIG_ALLOW_CROSS=1
      return 0
    fi
  fi

  if [[ -d "${pkg_config_libdir%%:*}" ]]; then
    if PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_LIBDIR="$pkg_config_libdir" pkg-config --exists "${required[@]}" >/dev/null 2>&1; then
      export PKG_CONFIG_ALLOW_CROSS=1
      export PKG_CONFIG_LIBDIR="$pkg_config_libdir"
      return 0
    fi
  fi

  release_warn "Skipping $label cross build: pkg-config sysroot for $triple is not configured"
  release_warn "Need target .pc files for: ${required[*]}"
  return 1
}

release_linux_export_appimage_env() {
  export NO_STRIP=1
  export APPIMAGE_EXTRACT_AND_RUN=1
  release_info "Linux: NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 (AppImage strip workaround)"
}

release_linux_tauri_build() {
  local -a args=("$@")
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

# Native build: deb + rpm + AppImage for the host CPU.
release_linux_build_native() {
  local cpu
  cpu="$(release_linux_cpu)"
  release_info "Linux native build ($cpu): deb, rpm, AppImage"
  release_linux_export_appimage_env
  release_linux_tauri_build
}

# Cross build: deb + rpm only (AppImage cannot be cross-built reliably).
release_linux_build_cross() {
  local triple="$1"
  local label="$2"
  if ! release_linux_cross_linker_ready "$triple"; then
    release_warn "Skipping $label cross build: linker for $triple not found (install cross gcc)"
    return 0
  fi
  if ! release_linux_cross_pkg_config_ready "$triple" "$label"; then
    return 0
  fi
  release_info "Linux cross build ($label / $triple): deb, rpm"
  release_linux_export_appimage_env
  release_linux_tauri_build --target "$triple" --bundles deb,rpm
}

release_linux_prepare_targets() {
  if [[ "${RELEASE_DRY_RUN:-0}" == "1" ]]; then
    release_info "Would: rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu"
    return 0
  fi
  rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu 2>/dev/null || true
}

release_linux_build_all() {
  local mode="${RELEASE_LINUX_ARCH:-all}"
  local cpu
  cpu="$(release_linux_cpu)"

  release_linux_prepare_targets

  case "$mode" in
    native)
      release_linux_build_native
      ;;
    amd64 | x86_64 | x64)
      if [[ "$cpu" == "amd64" ]]; then
        release_linux_build_native
      else
        release_linux_build_cross x86_64-unknown-linux-gnu amd64
        release_warn "AppImage amd64: run on an x86_64 Linux host (not cross-built here)"
      fi
      ;;
    arm64 | aarch64 | arm)
      if [[ "$cpu" == "arm64" ]]; then
        release_linux_build_native
      else
        release_linux_build_cross aarch64-unknown-linux-gnu arm64
        release_warn "AppImage arm64: run on an aarch64 Linux host or use CI arm runner"
      fi
      ;;
    all | both | *)
      release_linux_build_native
      if [[ "$cpu" == "amd64" ]]; then
        release_linux_build_cross aarch64-unknown-linux-gnu arm64
      else
        release_linux_build_cross x86_64-unknown-linux-gnu amd64
      fi
      ;;
  esac
}
