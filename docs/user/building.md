# Building BLXCode

This guide explains how to build BLXCode from source for Linux, macOS, and Windows.

## Which Build Command Should I Use?

For a runnable desktop application or installer, use:

```bash
cargo tauri build
```

Plain Cargo commands are still useful, but they do something different:

```bash
cargo build --workspace
```

This compiles Rust crates for development checks. It does not replace the full Tauri bundling pipeline, because BLXCode also needs the Trunk-built WebAssembly frontend and native app bundle metadata from `src-tauri/tauri.conf.json`.

## Common Requirements

Install these on every platform:

- Rust stable.
- Cargo.
- Trunk.
- Cargo Tauri CLI.
- The WebAssembly Rust target used by the Leptos frontend.
- System audio development libraries required by `cpal` on your platform.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
```

Then build from the repository root:

```bash
cargo tauri build
```

The build command runs `trunk build` automatically through `src-tauri/tauri.conf.json`, then bundles the native app.

Build artifacts are written under `target/release/bundle/` or `target/<triple>/release/bundle/` when building for an explicit Rust target.

## Linux

Install the Tauri system dependencies for your distribution before building.

For Debian or Ubuntu:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libasound2-dev \
  pkg-config
```

Then build:

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
cargo tauri build
```

Common Linux outputs include AppImage, Debian, and RPM bundles under `target/release/bundle/` (per CPU arch), for example:

- `deb/BLXCode_0.1.12_amd64.deb`, `rpm/BLXCode-0.1.12-1.x86_64.rpm`, `appimage/BLXCode_0.1.12_amd64.AppImage`
- `deb/BLXCode_0.1.12_arm64.deb`, `rpm/BLXCode-0.1.12-1.aarch64.rpm`, `appimage/BLXCode_0.1.12_aarch64.AppImage` (native on ARM64 Linux or CI `ubuntu-24.04-arm`)

Use `./scripts/release.sh --linux-arch amd64` or `--linux-arch arm64` to build only one architecture.

On Arch and other distros with newer binutils, AppImage bundling may fail during `linuxdeploy` strip with `.relr.dyn` errors. Use [`scripts/release.sh`](../../scripts/release.sh) (sets the variables automatically) or:

```bash
NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 cargo tauri build --bundles appimage
```

## macOS

**You cannot produce macOS `.app` / `.dmg` bundles on Linux or Windows.** Tauri links against Apple toolchains and must run `cargo tauri build` on a Mac (or use [GitHub Actions](https://github.com/tauri-apps/tauri-action) on `macos-latest` — see `.github/workflows/release.yml` in this repo).

Install Apple's build tools first:

```bash
xcode-select --install
```

If you plan to do iOS work too, install full Xcode instead of only Command Line Tools. For desktop-only BLXCode builds, Command Line Tools are usually enough.

BLXCode's voice recorder uses the system audio stack through `cpal`; no extra Homebrew package is normally required for macOS audio capture.

### Apple Silicon only (`aarch64`)

```bash
rustup target add wasm32-unknown-unknown aarch64-apple-darwin
cargo install trunk tauri-cli
cargo tauri build --target aarch64-apple-darwin
```

### Intel Mac only (`x86_64`)

```bash
rustup target add wasm32-unknown-unknown x86_64-apple-darwin
cargo install trunk tauri-cli
cargo tauri build --target x86_64-apple-darwin
```

### Universal binary (Apple Silicon + Intel in one `.app`)

On any Mac with both Rust targets installed:

```bash
rustup target add wasm32-unknown-unknown aarch64-apple-darwin x86_64-apple-darwin
cargo install trunk tauri-cli
cargo tauri build --target universal-apple-darwin
```

Typical outputs under `target/release/bundle/macos/` include `BLXCode.app` and, when enabled, a `.dmg` (names include the version, for example `BLXCode_0.1.8_universal.dmg`).

Unsigned local builds may be blocked or warned about by Gatekeeper on other Macs. Distribution builds should be signed and notarized with an Apple Developer account.

## Windows

Install:

- Microsoft C++ Build Tools with **Desktop development with C++** selected.
- Microsoft Edge WebView2 Runtime if it is not already installed.
- Rust with the MSVC toolchain.

The voice recorder uses Windows audio APIs through `cpal`, so use the MSVC Rust toolchain rather than GNU for the least surprising Tauri build path.

In PowerShell, make sure Rust uses the MSVC toolchain:

```powershell
rustup default stable-msvc
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
cargo tauri build
```

Windows bundle outputs usually include installer artifacts such as `.msi` or setup executables depending on the Tauri bundler configuration and installed tooling.

Because BLXCode currently uses `"targets": "all"` in `src-tauri/tauri.conf.json`, MSI packaging may require the Windows VBSCRIPT optional feature. If the build fails around `light.exe`, enable VBSCRIPT from Windows Optional Features and try again.

## Release script

From the repository root, [`scripts/release.sh`](../../scripts/release.sh) automates version bumps, CHANGELOG updates, signed `cargo tauri build`, and GitHub release uploads. Windows also has native PowerShell and Command Prompt entrypoints:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/release.ps1 --platform windows
scripts\release.cmd --platform windows
scripts\release-windows.cmd
```

macOS can use the generic Bash script or the explicit macOS launcher:

```bash
./scripts/release.sh --platform macos
./scripts/release-macos.sh
```

On Linux (including Arch), the script sets `NO_STRIP=1` and `APPIMAGE_EXTRACT_AND_RUN=1` so AppImage bundling avoids linuxdeploy strip errors with `.relr.dyn` sections.

For a **local bundle build only**, you do not need `.env.release`. Run:

```bash
./scripts/release.sh
```

Builds are **unsigned by default** (no Apple/Windows code signing, no Tauri updater key). Use `--require-signing` only when `.env.release` has `TAURI_SIGNING_PRIVATE_KEY`.

That builds Linux bundles for **amd64 and arm64** by default (`--linux-arch all`): native deb/rpm/AppImage on your CPU, cross-built deb/rpm for the other arch (AppImage for the non-native arch needs an ARM runner or CI — see below). macOS uses **`universal-apple-darwin`** (one `.app`/`.dmg` for Apple Silicon and Intel). Artifacts land under `target/**/release/bundle/`.

For cross-built Linux bundles, the cross linker alone is not enough for GTK/WebKit apps. You also need a target pkg-config sysroot or wrapper with target `.pc` files for `glib-2.0`, `gobject-2.0`, `gio-2.0`, `gtk+-3.0`, and `webkit2gtk-4.1`. On Arch amd64, `sudo pacman -S aarch64-linux-gnu-gcc` installs the linker, but you still need an arm64 sysroot or an `aarch64-linux-gnu-pkg-config` wrapper that resolves the arm64 GTK/WebKit packages. Without that, the release script skips arm64 and prints the missing pkg-config setup instead of failing after the native build.

Optional signing and upload variables are documented in [`.env.release.example`](../../.env.release.example) at the repo root (copy to `.env.release`, which is gitignored).

```bash
./scripts/release.sh --help
powershell -ExecutionPolicy Bypass -File scripts/release.ps1 --help

# Signed bundles + .sig (requires .env.release)
./scripts/release.sh --require-signing
scripts\release.cmd --require-signing

# New patch release: bump versions + CHANGELOG, build, draft upload
./scripts/release.sh --bump patch --build --upload
scripts\release.cmd --bump patch --build --upload

# Trigger CI for all platforms (after commit)
./scripts/release.sh --bump patch --tag --push --no-build --commit

# CI already created v0.1.10 — add Linux artifacts from Arch
./scripts/release.sh --build --upload

# Upload existing target/release/bundle files only
./scripts/release.sh --upload-only
```

If a GitHub release or tag `v{X.Y.Z}` already exists for the version in `src-tauri/tauri.conf.json`, `--upload` attaches new bundle files only (skips duplicate asset names unless you pass `--clobber`).

## Cross-Platform Builds

For the least painful path, build each platform on that platform:

- Build Linux packages on Linux (`.deb`, `.rpm`, `.AppImage`).
- Build macOS `.app` / `.dmg` on macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`, or `universal-apple-darwin`).
- Build Windows installers on Windows.

Cross-compiling Tauri desktop bundles from Linux to macOS is **not supported** for release artifacts. Use a Mac or the repository **Release** workflow to produce binaries in CI:

- **Tag push** (`v*`) — builds all platforms (Linux, macOS universal, Windows).
- **Manual run** (`workflow_dispatch`) — choose **Alle**, **Linux (deb, rpm, AppImage)**, **Mac Universal**, or **Windows**.

Only the **repository owner** may trigger that workflow (on org-owned repos, set the Actions variable `RELEASE_OWNER` to the allowed GitHub login).

## Clean Rebuild

If a build gets into a strange state, remove generated output and rebuild:

```bash
cargo clean
rm -rf dist
cargo tauri build
```

On Windows PowerShell:

```powershell
cargo clean
Remove-Item -Recurse -Force dist
cargo tauri build
```

## Reference

BLXCode follows the standard Tauri 2 desktop build flow. See the official Tauri prerequisites and platform installer docs for the most current OS dependency details:

- https://v2.tauri.app/start/prerequisites/
- https://v2.tauri.app/distribute/windows-installer/
