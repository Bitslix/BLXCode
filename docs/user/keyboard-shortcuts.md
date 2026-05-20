# Keyboard Shortcuts

BLXCode supports two shortcut modes: **Tmux-style** prefix chords (default) and **Legacy** direct chords. The welcome screen and this page reflect the active mode.

Change the mode in **BLXCode Settings** → **App** → **Keyboard shortcuts** (`blxcode_shortcut_mode_v1`: `tmux` or `legacy`).

## Tmux mode (default)

Press **Ctrl+b**, then a second key within **1.5 seconds**. Press **Esc** to cancel an armed prefix.

While a workspace **terminal** has focus, **Ctrl+b** is not intercepted — the shell keeps the prefix (for example tmux inside the PTY).

| Second key | Action |
|------------|--------|
| `o` | Quick Open |
| `p` | Toggle right panel |
| `a` | Agent tab |
| `b` | Browser tab |
| `m` | Memory tab |
| `n` | New terminal slot (active workspace) |
| `:` | Command palette |

## Legacy mode

Direct chords (no prefix):

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Quick Open |
| `Ctrl+P` | Toggle right panel |
| `Ctrl+Shift+A` | Agent tab |
| `Ctrl+Shift+B` | Browser tab |
| `Ctrl+Shift+M` | Memory tab |
| `` Ctrl+` `` | New terminal slot (active workspace) |
| `Ctrl+Shift+P` | Command palette |

## Other shortcuts

These are unchanged by shortcut mode:

- **Ctrl+Shift+P** (command palette entry) also opens **BLXCode settings** from the palette when not using the tmux `:` binding in legacy mode — use the palette's settings action or the gear control.
- Right-panel tabs **Plans**, **Rules**, and **Skills** are reachable from the tab strip; there are no default tmux second-keys for them yet.

## Notifications (handoff feedback)

**BLXCode Settings** → **App** → **Notifications**:

- **Show success toasts** (`blxcode_success_toast_v1`) — brief bottom-right confirmation when context is sent or attached.
- **Play success sound** (`blxcode_success_sound_v1`) — short tone on successful handoff.

Errors always show an error toast regardless of the success-toast toggle.

## See also

- [Getting Started](getting-started.md) — first launch and welcome screen
- [Workspaces](workspaces.md) — terminal focus and handoff
- [Agent Providers](agent-providers.md) — agent panel and settings
