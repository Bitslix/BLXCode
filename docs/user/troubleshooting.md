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

BLXCode stores keys from **Settings → API Keys** in the OS keyring when available. If the keyring is unavailable, it falls back to app-config secret files.

On Linux, make sure a secret service such as GNOME Keyring or KWallet is available if you want keyring-backed storage. Otherwise, check app config directory permissions.

## Agent Says Provider Key Is Missing

Open **Settings → API Keys**, set the key for the provider shown in the error (OpenRouter, Anthropic, OpenAI, etc.), click **Save**, then retry. Keys are per provider — an Anthropic key does not configure OpenRouter.

## Web Search Or Fetch Unavailable

`web_search` and `web_fetch` are omitted from the tool list until a web backend is configured.

1. Set Tavily and/or Brave keys under **Settings → API Keys** (or `BLX_TAVILY_API_KEY` / `BLX_BRAVE_API_KEY` in the environment).
2. Open **Settings → BLXCode Agent** → **Web Tools**, choose **Tavily** or **Brave**, and **Save** the agent footer.
3. Enable the **web** core skill in the Skills panel if it was disabled.

See [Agent Harness](agent-harness.md).

## Windows: Flashing Console Windows Or Frozen UI With Git Open

On Windows, older builds could flash a console window for every short Git subprocess, or feel frozen while File Diff / Git Commits refreshed — especially in repos where `cargo tauri dev` keeps writing to `target/`.

**0.3.1 and later** hide those subprocess windows, run blocking Git work off the UI thread, and ignore build/dependency folders in the status watcher. Update to the latest release if you still see the issue.

## Linux: Window Stutters While Git Sidebar Is Active

If the workbench stutters when dragging the window during active Git refreshes (common during `cargo tauri dev` with a `target/` directory), upgrade to **0.3.1+** — Git commands no longer block the main thread, and the watcher skips `target/`, `node_modules/`, and similar paths.

## Shell Or Git Tool Says Call environment_detect First

The agent must run `environment_detect` once per workspace session before `shell_exec` or Git tools. Switching workspaces clears the cache — start a new turn after switching so the agent can detect again.

If the error persists in the same workspace, send a short prompt such as “detect environment for this workspace” and retry.

## Subagents Not Appearing

Subagents run only when you **explicitly** ask (for example “use subagents to review …”). The coordinator does not spawn them automatically. You need a configured provider key and a model that supports tool calling.

Full behaviour, roles, and limits: [Subagents](subagents.md).

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
