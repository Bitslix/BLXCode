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

Common Linux outputs include AppImage, Debian, or RPM-style bundle folders depending on the Tauri bundler setup and installed tools.

## macOS

Install Apple's build tools first:

```bash
xcode-select --install
```

If you plan to do iOS work too, install full Xcode instead of only Command Line Tools. For desktop-only BLXCode builds, Command Line Tools are usually enough.

BLXCode's voice recorder uses the system audio stack through `cpal`; no extra Homebrew package is normally required for macOS audio capture.

Then build:

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
cargo tauri build
```

Typical macOS outputs include an `.app` bundle and, when configured by Tauri, a `.dmg`.

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

- Build Linux packages on Linux.
- Build macOS `.app` or `.dmg` artifacts on macOS.
- Build Windows installers on Windows.

Cross-compiling Tauri apps is possible in some cases, especially Windows NSIS builds from Linux/macOS, but it has more moving parts and is best handled later through CI.

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
