#!/usr/bin/env bash
# Interactive BLXCode setup for Linux development and local desktop builds.
set -Eeuo pipefail

YES=0
CHECK_ONLY=0
SKIP_SYSTEM=0
NO_VERIFY=0
WITH_BUNDLE=0
WARNINGS=()

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ -t 1 ]] && command -v tput >/dev/null 2>&1; then
  BOLD="$(tput bold)"
  DIM="$(tput dim)"
  RED="$(tput setaf 1)"
  GREEN="$(tput setaf 2)"
  YELLOW="$(tput setaf 3)"
  BLUE="$(tput setaf 4)"
  RESET="$(tput sgr0)"
else
  BOLD=""
  DIM=""
  RED=""
  GREEN=""
  YELLOW=""
  BLUE=""
  RESET=""
fi

usage() {
  cat <<'EOF'
Usage: ./scripts/setup/setup-linux.sh [options]

Set up BLXCode after a git clone on Linux. Supports apt, dnf, and pacman.

Options:
  --yes          Accept install prompts.
  --check-only   Inspect and print planned actions without installing or building.
  --skip-system  Skip OS package-manager installs.
  --no-verify    Install/check prerequisites but skip cargo/trunk verification.
  --with-bundle  Run cargo tauri build after verification.
  -h, --help     Show this help.
EOF
}

info() { printf '%s\n' "${BLUE}info${RESET}  $*"; }
ok() { printf '%s\n' "${GREEN}ok${RESET}    $*"; }
warn() {
  WARNINGS+=("$*")
  printf '%s\n' "${YELLOW}warn${RESET}  $*"
}
die() {
  printf '%s\n' "${RED}error${RESET} $*" >&2
  exit 1
}
section() {
  printf '\n%s\n' "${BOLD}==> $*${RESET}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes) YES=1; shift ;;
    --check-only) CHECK_ONLY=1; shift ;;
    --skip-system) SKIP_SYSTEM=1; shift ;;
    --no-verify) NO_VERIFY=1; shift ;;
    --with-bundle) WITH_BUNDLE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "Unknown option: $1 (use --help)" ;;
  esac
done

confirm() {
  local prompt="$1"
  if [[ "$YES" == "1" ]]; then
    return 0
  fi
  if [[ ! -t 0 ]]; then
    warn "Skipping prompt in non-interactive shell: $prompt"
    return 1
  fi
  local answer
  read -r -p "$prompt [y/N] " answer
  [[ "$answer" =~ ^[Yy]$|^[Yy][Ee][Ss]$ ]]
}

print_command() {
  printf '  %s' "${DIM}+${RESET}"
  printf ' %q' "$@"
  printf '\n'
}

run() {
  print_command "$@"
  if [[ "$CHECK_ONLY" == "1" ]]; then
    return 0
  fi
  "$@"
}

as_root() {
  if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    run "$@"
  else
    run sudo "$@"
  fi
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

add_cargo_bin_to_path() {
  local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  case ":$PATH:" in
    *":$cargo_bin:"*) ;;
    *) export PATH="$cargo_bin:$PATH" ;;
  esac
}

check_platform() {
  if [[ "$(uname -s)" != "Linux" ]]; then
    die "This script is for Linux. Use scripts/setup-macos.sh on macOS or scripts/setup-windows.ps1 on Windows."
  fi
}

detect_pkg_manager() {
  if command_exists apt-get; then
    printf 'apt\n'
  elif command_exists dnf; then
    printf 'dnf\n'
  elif command_exists pacman; then
    printf 'pacman\n'
  else
    printf 'unknown\n'
  fi
}

print_linux_dependency_help() {
  cat <<'EOF'

Install one of these dependency sets manually, then re-run this script:

apt:
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
    libasound2-dev pkg-config patchelf nodejs npm

dnf:
  sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
    libappindicator-gtk3-devel librsvg2-devel libxdo-devel alsa-lib-devel \
    pkgconf-pkg-config patchelf nodejs npm
  sudo dnf group install "c-development"

pacman:
  sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
    appmenu-gtk-module libappindicator-gtk3 librsvg xdotool alsa-lib \
    pkgconf patchelf nodejs npm
EOF
}

install_system_dependencies() {
  section "System dependencies"
  if [[ "$SKIP_SYSTEM" == "1" ]]; then
    warn "Skipping Linux package-manager setup (--skip-system)."
    print_linux_dependency_help
    return
  fi

  local manager
  manager="$(detect_pkg_manager)"
  case "$manager" in
    apt)
      local packages=(
        libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev
        libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev
        pkg-config patchelf nodejs npm
      )
      if confirm "Install Tauri/Linux packages with apt?"; then
        as_root apt-get update
        as_root apt-get install -y "${packages[@]}"
      else
        warn "System package install skipped."
      fi
      ;;
    dnf)
      local packages=(
        webkit2gtk4.1-devel openssl-devel curl wget file
        libappindicator-gtk3-devel librsvg2-devel libxdo-devel alsa-lib-devel
        pkgconf-pkg-config patchelf nodejs npm
      )
      if confirm "Install Tauri/Linux packages with dnf?"; then
        as_root dnf install -y "${packages[@]}"
        as_root dnf group install -y "c-development"
      else
        warn "System package install skipped."
      fi
      ;;
    pacman)
      local packages=(
        webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module
        libappindicator-gtk3 librsvg xdotool alsa-lib pkgconf patchelf nodejs npm
      )
      if confirm "Install Tauri/Linux packages with pacman?"; then
        as_root pacman -Syu --needed --noconfirm "${packages[@]}"
      else
        warn "System package install skipped."
      fi
      ;;
    *)
      warn "Could not detect apt, dnf, or pacman."
      print_linux_dependency_help
      ;;
  esac
}

ensure_rust() {
  section "Rust toolchain"
  add_cargo_bin_to_path

  if command_exists rustup && command_exists cargo; then
    ok "Rust is available: $(cargo --version)"
    return
  fi

  warn "Rust/Cargo was not found in PATH."
  if [[ "$CHECK_ONLY" == "1" ]]; then
    info "Would offer rustup install: curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y"
    return
  fi
  if ! command_exists curl; then
    die "curl is required to install rustup. Install curl and re-run this script."
  fi
  if confirm "Install Rust stable with rustup?"; then
    run sh -c "curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y"
    add_cargo_bin_to_path
  else
    warn "Rust install skipped. Install Rust from https://www.rust-lang.org/tools/install and re-run."
  fi
}

node_major_version() {
  node -p "Number(process.versions.node.split('.')[0])" 2>/dev/null || printf '0\n'
}

ensure_node() {
  section "Node and npm"
  if ! command_exists node || ! command_exists npm; then
    warn "Node.js and npm are required for frontend-js."
    warn "Install Node.js LTS. Node 22 is recommended because CI uses it."
    return
  fi

  local major
  major="$(node_major_version)"
  if [[ "$major" -lt 18 ]]; then
    die "Node.js >= 18 is required; found $(node --version). Install Node.js LTS, preferably 22."
  fi

  ok "Node is available: $(node --version) (Node 22 recommended)"
  ok "npm is available: $(npm --version)"
}

cargo_tauri_available() {
  cargo tauri --version >/dev/null 2>&1
}

ensure_cargo_tools() {
  section "Cargo targets and tools"
  if ! command_exists rustup || ! command_exists cargo; then
    warn "Skipping Cargo tool setup because rustup/cargo is not available."
    return
  fi

  run rustup target add wasm32-unknown-unknown

  if command_exists trunk; then
    ok "Trunk is available: $(trunk --version)"
  else
    run cargo install trunk --locked
    add_cargo_bin_to_path
  fi

  if cargo_tauri_available; then
    ok "Cargo Tauri CLI is available: $(cargo tauri --version)"
  else
    run cargo install tauri-cli --version "^2" --locked
    add_cargo_bin_to_path
  fi
}

run_frontend_setup() {
  section "Frontend JavaScript dependencies"
  if ! command_exists npm; then
    warn "Skipping npm setup because npm is not available."
    return
  fi

  run npm ci --prefix "$ROOT/frontend-js"
  run npm --prefix "$ROOT/frontend-js" run build:graph3d
}

run_verification() {
  if [[ "$NO_VERIFY" == "1" ]]; then
    section "Verification"
    warn "Skipping verification (--no-verify)."
    return
  fi

  section "Verification"
  (cd "$ROOT" && run cargo check -p blxcode)
  (cd "$ROOT" && run cargo check -p blxcode-ui --target wasm32-unknown-unknown)
  (cd "$ROOT" && run cargo test --workspace)
  (cd "$ROOT" && run trunk build)

  if [[ "$WITH_BUNDLE" == "1" ]]; then
    section "Bundle build"
    (cd "$ROOT" && run cargo tauri build)
  fi
}

summary() {
  section "Summary"
  info "Repo root: $ROOT"
  if [[ "$CHECK_ONLY" == "1" ]]; then
    info "Mode: check-only; no installs or builds were executed."
  fi
  if [[ ${#WARNINGS[@]} -gt 0 ]]; then
    printf '%s\n' "${YELLOW}warn${RESET}  Completed with ${#WARNINGS[@]} warning(s):"
    local item
    for item in "${WARNINGS[@]}"; do
      printf '  - %s\n' "$item"
    done
  else
    ok "Setup completed without warnings."
  fi
}

main() {
  check_platform
  section "BLXCode Linux setup"
  info "This script sets up Rust, Tauri, Trunk, Node/npm, and Linux native dependencies."
  info "Default verification does not launch the app. Use --with-bundle for cargo tauri build."

  install_system_dependencies
  ensure_rust
  ensure_node
  ensure_cargo_tools
  run_frontend_setup
  run_verification
  summary
}

main "$@"
