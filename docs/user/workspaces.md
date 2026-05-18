# Workspaces

A BLXCode workspace is a project folder plus the UI state needed to work inside it: terminal grid, split panes, assigned agent labels, agent timeline, embedded browser tabs, recent workspace data, and right-panel layout.

## Workspace Creation

The workspace configurator lets you:

- Select or create a project directory.
- Type `cd ...` style navigation commands for fast path movement.
- Pick a terminal-grid preset.
- Assign terminal slots to a fleet of coding tools.
- Skip agent assignment when you only want plain terminals.

The supported fleet labels are:

- `claude`
- `codex`
- `gemini`
- `opencode`
- `cursor`

## Terminal Grids And Panes

Each workspace has a top-level terminal grid. Preset counts map to balanced grid dimensions:

| Terminals | Grid |
|---:|:---|
| 1 | 1 x 1 |
| 2 | 1 x 2 |
| 4 | 2 x 2 |
| 6 | 2 x 3 |
| 8 | 2 x 4 |
| 9 | 3 x 3 |
| 12 | 3 x 4 |
| 16 | 4 x 4 |

Individual terminal slots can also keep split-pane state. BLXCode persists pane IDs, split axis, and terminal layout so the workbench can restore the surface after restart.

## Shell Environment

The backend spawns PTY sessions through `portable-pty`. On Unix-like systems it uses `$SHELL`, falling back to `/bin/sh`.

BLXCode injects a few environment variables into terminal sessions when needed:

- `BLX_TERMINAL_KEY`: stable terminal/session mapping key.
- `BLX_AGENT_SLUG`: assigned agent label for the slot.
- `BLX_SESSIONS_PATH`: app-managed session mapping file path.

These values support session capture hooks for external coding tools.

## Embedded Browser

BLXCode captures HTTP and HTTPS links from markdown, terminal integration events, and DOM clicks, then opens them in the embedded browser area.

On Windows and macOS, the backend can use native child webviews through Tauri unstable APIs. On Linux, BLXCode falls back to an iframe-based embedded surface because native child inset support is disabled.

Some websites block iframe embedding through `X-Frame-Options` or `Content-Security-Policy: frame-ancestors`. BLXCode probes these headers and can route around blocked embeds when possible.

## Persistence

Workbench state is saved through Tauri commands with a short debounce. BLXCode also performs a best-effort save when the window is closing.

Persisted state includes:

- Open workspaces.
- Active workspace.
- Recent workspaces.
- Sidebar and right-panel collapsed state.
- Right-panel width and active tab.
- Embedded browser tabs.
- Workspace terminal and pane layout.
- Agent timeline and compose draft.

If a saved snapshot has an unsupported schema version, BLXCode ignores it and starts with defaults rather than crashing.
