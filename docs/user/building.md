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

Common Linux outputs include AppImage, Debian, and RPM bundles under `target/release/bundle/` (for example `deb/BLXCode_0.1.8_amd64.deb`, `rpm/BLXCode-0.1.8-1.x86_64.rpm`, `appimage/BLXCode_0.1.8_amd64.AppImage`).

On Arch and other distros with newer binutils, AppImage bundling may fail during `linuxdeploy` strip with `.relr.dyn` errors. Use:

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
