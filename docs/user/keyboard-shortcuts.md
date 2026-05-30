# Keyboard Shortcuts

BLXCode ships two shortcut **presets**: **Tmux-style** prefix chords (default) and **Classic** direct chords. Every binding is also individually **rebindable**.

Manage everything in **BLXCode Settings** → **Shortcuts**:

- **Preset** — Tmux or Classic. Selecting a preset fills in its default bindings.
- **Prefix key** — the chord prefix used by tmux-style bindings (default **Ctrl+B**). Click **Rebind** and press a new combination.
- **Bindings** — per action: the current key, a **Rebind** button (press a key combination; rebinding a tmux chord changes its second key, rebinding a direct combo changes the whole combo), and a **Reset** button. **Reset all to preset** restores the current preset's defaults.

Bindings persist in `localStorage` (`blxcode_shortcut_bindings_v1`); the chosen preset is stored in `blxcode_shortcut_mode_v1`.

## Chords are always captured

Shortcut chords fire **even while a workspace terminal has focus** — so `Ctrl+b` `n` opens a new terminal from a plain shell instead of sending `^B` to it. Typing into other app inputs (agent box, search fields) is never intercepted.

> If you run real **tmux** inside a PTY, its prefix also defaults to `Ctrl+b`. Rebind the BLXCode prefix (e.g. to `Ctrl+a`) in **Settings → Shortcuts** so the two don't collide.

## Tmux preset (default)

Press the **prefix** (**Ctrl+b**), then a second key within **1.5 seconds**. Press **Esc** to cancel an armed prefix.

| Second key | Action |
| ---------- | ------ |
| `o` | Quick Open |
| `r` | Toggle right panel |
| `a` | Agent tab |
| `b` | Browser tab |
| `m` | Memory tab |
| `n` | New terminal slot (active workspace) |
| `p` | Command palette |

## Classic preset

Direct chords (no prefix):

| Shortcut | Action |
| -------- | ------ |
| `Ctrl+O` | Quick Open |
| `Ctrl+P` | Toggle right panel |
| `Ctrl+Shift+A` | Agent tab |
| `Ctrl+Shift+B` | Browser tab |
| `Ctrl+Shift+M` | Memory tab |
| `Ctrl+Shift+N` | New terminal slot (active workspace) |
| `Ctrl+Shift+P` | Command palette |

## Notifications (handoff feedback)

**BLXCode Settings** → **App** → **Notifications**:

- **Show success toasts** (`blxcode_success_toast_v1`) — brief bottom-right confirmation when context is sent or attached.
- **Play success sound** (`blxcode_success_sound_v1`) — short tone on successful handoff.

Errors always show an error toast regardless of the success-toast toggle.

## See also

- [Getting Started](getting-started.md) — first launch and welcome screen
- [Workspaces](workspaces.md) — terminal focus and handoff
- [Agent Providers](agent-providers.md) — agent panel and settings
