# Troubleshooting

## `cargo tauri dev` Cannot Find Trunk

Install Trunk and make sure Cargo's bin directory is on your path:

```bash
cargo install trunk
export PATH="$HOME/.cargo/bin:$PATH"
```

The Tauri config already prepends `$HOME/.cargo/bin` when running the frontend dev/build commands.

## Missing WASM Target

If the frontend crate cannot compile for WebAssembly, install the target:

```bash
rustup target add wasm32-unknown-unknown
```

## Linux Blank Window Or WebKit Rendering Problems

On Linux, BLXCode sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` unless you already set that environment variable. This works around common WebKit2GTK blank-window and GBM renderer issues.

If problems continue, verify your WebKitGTK, GPU driver, and Tauri Linux dependencies are installed correctly.

## Embedded Browser Does Not Load A Site

Some sites block embedding with `X-Frame-Options` or `Content-Security-Policy` headers. BLXCode probes for common blockers, but not every site can be embedded.

Try opening another URL or use your system browser for sites that explicitly deny embedding.

## API Key Does Not Save

BLXCode stores provider keys in the OS keyring when available. If the keyring is unavailable, it falls back to app-config secret files.

On Linux, make sure a secret service such as GNOME Keyring or KWallet is available if you want keyring-backed storage. Otherwise, check app config directory permissions.

## Agent Says Provider Key Is Missing

Open provider settings, select the provider you want to use, paste an API key, save it, and retry the agent turn. Provider keys are stored per provider, so saving an Anthropic key does not configure OpenRouter or OpenAI.

## Terminal Starts In The Wrong Directory

Check the workspace folder in the sidebar or workspace configurator. Terminal sessions spawn from the workspace `cwd`. If no workspace is selected, BLXCode uses a default sandbox under the app data directory.

## State Looks Corrupt After A Change

Workbench snapshots are versioned. Unsupported versions are ignored, but partially corrupted app state can still produce odd UI. If needed, clear the app config/data files for BLXCode from your platform's application data directory and restart.
