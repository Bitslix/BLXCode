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

## Voice Recording Cannot Start

Check that your system has a default input device and that BLXCode has microphone permission. In development, your OS may grant microphone permission to the terminal, shell, or built app wrapper rather than to a named BLXCode release bundle.

On Linux, verify your audio stack is running and that the user session can access the microphone through PipeWire, PulseAudio, ALSA, or your distribution's configured audio backend.

## STT Fails

Make sure the selected STT provider has an API key saved in provider settings. OpenAI STT uses the OpenAI key; OpenRouter STT uses the OpenRouter key.

If language hints seem wrong, switch voice settings from **Follow app** to **Auto detect** or enter a manual ISO language code.

## TTS Does Not Play

TTS currently supports OpenAI speech synthesis. Save an OpenAI key, use an OpenAI TTS model such as `gpt-4o-mini-tts`, and keep TTS autoplay enabled in voice settings.

Some browsers/webviews block autoplay in edge cases. Interact with the app once and retry the voice turn.

## Terminal Starts In The Wrong Directory

Check the workspace folder in the sidebar or workspace configurator. Terminal sessions spawn from the workspace `cwd`. If no workspace is selected, BLXCode uses a default sandbox under the app data directory.

## State Looks Corrupt After A Change

Workbench snapshots are versioned. Unsupported versions are ignored, but partially corrupted app state can still produce odd UI. If needed, clear the app config/data files for BLXCode from your platform's application data directory and restart.
