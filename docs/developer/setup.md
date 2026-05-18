# Developer Setup

BLXCode is a two-crate Rust workspace:

| Crate | Path | Purpose |
|---|---|---|
| `blxcode-ui` | `src/` | Leptos CSR frontend compiled to WASM by Trunk. |
| `blxcode` | `src-tauri/` | Tauri 2 backend, native commands, state, PTY, provider clients. |

## Prerequisites

Install:

- Rust stable.
- `wasm32-unknown-unknown` target.
- Trunk.
- Cargo Tauri CLI.
- Tauri 2 native system dependencies.
- Native audio development dependencies used by the voice recorder.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
```

On Debian or Ubuntu, the current Linux dependency set includes Tauri's WebKitGTK packages plus ALSA headers for `cpal` voice recording:

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

## Development Commands

```bash
cargo tauri dev
```

Runs the full desktop app. The Tauri config starts Trunk automatically.

```bash
trunk serve
```

Runs the frontend only at `http://localhost:1420`. Tauri-only features are unavailable in this mode, but it can be useful for UI iteration.

## Verification Commands

```bash
cargo test --workspace
cargo check -p blxcode
cargo check -p blxcode-ui --target wasm32-unknown-unknown
trunk build
```

Use the narrowest command while iterating, then run the broader checks before publishing a change.

## Build

```bash
cargo tauri build
```

Bundle configuration lives in `src-tauri/tauri.conf.json`.

## Important Config Files

- `Cargo.toml`: workspace and frontend crate manifest.
- `src-tauri/Cargo.toml`: backend crate manifest.
- `Trunk.toml`: frontend target, watch ignores, and dev server port.
- `src-tauri/tauri.conf.json`: Tauri app identity, dev/build commands, bundle resources, and window config.
- `src-tauri/capabilities/default.json`: Tauri v2 permissions.
- `src/config/app.config.rs`: frontend constants and local storage keys.

## Generated Or External Assets

Bundled hook scripts live under `content/hooks/` and are included as Tauri bundle resources. EULA markdown lives under `content/eula/` and is compiled into the frontend.
