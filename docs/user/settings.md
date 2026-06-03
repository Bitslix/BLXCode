# Settings

BLXCode opens settings in a **center workbench tab** (not a modal). The command palette entry **Open Settings** focuses an existing settings tab or creates one per workspace — see [Workspaces → Center tabs](workspaces.md#center-tabs) for the tab lifecycle. Settings can be opened even **without an active workspace** — BLXCode lazily provisions an ephemeral shell workspace that hosts only the Settings tab and is disposed when you close it.

## Sidebar categories

| Category | What it configures |
|----------|-------------------|
| **App** | UI language, STT language + push-to-talk, notifications, terminal hooks, app updates (Stable/Beta channel, background checks, structured release-notes dialog) |
| **Appearance** | App themes — 32 presets, search, Dark/Light filters, plus theme-independent **Roundings** and **Font** controls; see [Appearance & Themes](appearance-themes.md) |
| **Shortcuts** | Keyboard shortcut preset, prefix key, and per-action rebinding (incl. **Push-to-Talk**); see [Keyboard Shortcuts](keyboard-shortcuts.md) |
| **API Keys** | All provider secrets in one pane — see below |
| **Workspace** | Default project directory, agent sandbox root, embedded browser URL, **category colors** for Memory, **terminal naming** mode and name pool, confirm-before-closing |
| **BLXCode Agent** | Text, image, and voice inference; **Auto-compact** threshold; **tool-loop limit**; **Agent orb** (3D / 2D) — see below |
| **Remote** | SSH connection presets for remote workspaces (host/port/user, password / key / agent auth, encrypted secrets, session-resume model); see [Remote (SSH)](remote-ssh.md) |

Legacy saved categories (`Image`, `Voice`, `Memory`) still open the correct pane.

## App

**Settings → App** collects shell-wide preferences that aren't tied to a single workspace: UI language, voice/STT defaults, push-to-talk, notification toasts and sounds, terminal hooks, and the GitHub Releases auto-updater. Keyboard shortcuts moved to their own **Shortcuts** category (see [Keyboard Shortcuts](keyboard-shortcuts.md)).

App updates support two channels:

- **Stable** uses final GitHub Releases.
- **Beta** includes GitHub prereleases such as `0.6.0-pre.ed4dc`, and also accepts newer final stable releases when they supersede the current beta.

When **Check for updates on startup** is enabled, BLXCode checks immediately after launch and then every 10 minutes while the app is open. Background checks are quiet when nothing changed, show a discreet statusline item only during the check, and add a titlebar/native notification when a new update is available. Clicking that notification opens the update dialog.

The update dialog reuses the structured release-notes view from `post_update_release_notes(version, channel)` (hero summary, sections, loading state, fallback to the updater manifest body) instead of showing the manifest body as plain text — the update-specific controls (current → available version, install/download progress, retry, restart, **Later**) stay in place. Beta builds prefer `docs/releases/v{version}.md` for the exact prerelease tag and fall back to the stable base notes when needed.

<p align="center">
  <img src="../images/settings-app.png" alt="Settings → App pane with UI Language (English), Input language (Follow app language / Auto-detect / Manual), Keyboard shortcuts (Tmux style / Classic), push-to-talk toggle, Notifications (Show success toasts, Play success sound), Terminal hooks for claude/codex/gemini/cursor/opencode with Install hooks button, and App updates (Check for updates on startup, Current version 0.2.3)" />
</p>

## Appearance

**Settings → Appearance** lets you pick an app theme and adjust two theme-independent knobs:

- **Roundings** — a global corner-radius scale (Sharp / Default / Rounded / Extra) that re-rounds the whole workbench instantly.
- **Font** — a curated monospace picker (JetBrains Mono bundled, plus Cascadia Code, Fira Code, SF Mono, Menlo, Consolas, and System Monospace as system-dependent options). The xterm terminals read their `fontFamily` from the same token and re-fit when the font changes.
- **BLXCode** (default) — the redesigned Tokyo Night × Dracula dark workbench look.
- **BLXCode Legacy / BLXCode Legacy Light** — the previous GitHub-blue default, kept under explicit ids.
- Thirty additional dark/light presets (Dracula, Gruvbox, Solarized, Nord, One Dark/Light, Catppuccin, Tokyo Night & Light, Rosé Pine / Dawn, Everforest, Kanagawa, **Claude Code** warm-charcoal, Night Owl, Ayu Mirage / Light, GitHub Light, plus a family of cool light themes — Winter, Paper, Alpine, Frost, Lilac).
- Search and **All / Dark / Light** filters (16 dark, 16 light).
- Instant preview on each card; choice persists across restarts.

Themes affect sidebar, panels, terminals, graphs, and settings chrome. Embedded web pages, native webviews, and your Memory category color swatches are documented exceptions.

Full guide: [Appearance & Themes](appearance-themes.md).

## API Keys

**Settings → API Keys** is the only place to enter provider secrets.

- LLM providers: OpenRouter, Anthropic, OpenAI, and coming-soon rows (Google, Mistral, Grok xAI).
- Media / search: Tavily, Brave, **fal.ai** (image), **Amazon Polly** (AWS voice).
- One **Save** / **Discard** footer for the whole pane; per-row remove marks keys for deletion on save.
- Keys use the OS keyring (`BLXCode` service) with `BLX_*` env fallback when the store is empty; the UI shows **via env** when a fallback is active.

Agent, image, and voice panes show a short status line pointing here — they do not contain password fields.

<p align="center">
  <img src="../images/settings-api-keys.png" alt="Settings → API Keys pane with LLM Providers (OpenRouter configured ********4b6e, Anthropic NOT SET with BLX_ANTHROPIC_API_KEY env fallback hint, OpenAI NOT SET, Google/Mistral/Grok xAI COMING SOON), Search Providers (Tavily NOT SET, Brave Search NOT SET), and Image/Video/Voice section with fal.ai row" />
</p>

## BLXCode Agent

**Settings → BLXCode Agent** uses a grid:

| Area | Settings |
|------|----------|
| **Text** | Provider, thinking level, **tool-loop limit (1–500, default 36)**, model (`AgentModelPicker`), refresh |
| **Auto-compact** | Toggle (default on) and threshold (50–95 %, default 85 %) — runs a non-tool summarization pass when the context window crosses the threshold, between turns, at most once per crossing |
| **Image** | Provider, quality level, model, auto-save |
| **Voice** | Provider (OpenAI / OpenRouter / AWS), STT + TTS models, recording quality, post-STT behavior, voice picks, speak replies, **Push-to-Talk** mode, target, target mode, live partial transcript, TTS-collision policy |
| **Agent orb** | **3D Drobo** (default) or **2D logo** — pick which mode the voice orb renders in |
| **Web Tools** | Tavily / Brave / disabled backend |

One **Save** / **Discard** at the bottom persists text provider, tool-loop limit, auto-compact, and web tools together. Image and voice sections auto-save on change. The **Agent orb** switch updates the open Agent tab without restarting.

<p align="center">
  <img src="../images/settings-blxcode-agent.png" alt="Settings → BLXCode Agent pane with Text card (Provider OpenRouter, Thinking level Medium, Model openai/gpt-5 with pricing $1.25 in / $10.00 out per 1M tokens), Image card (Provider OpenRouter, Quality level Medium, Model google/gemini-2.5-flash-image with pricing $0.30 in / $2.50 out), Voice card (Provider OpenRouter, STT model gpt-4o-mini-transcribe, TTS model neural, recording quality Low / Standard / High, post-STT behavior, 6-voice picker grid, Speak agent replies toggle), and Web Tools row (Disabled / Tavily / Brave)" />
</p>

Details: [Agent Providers](agent-providers.md), [Image Mode](image.md), [Voice](voice.md).

## Workspace

**Settings → Workspace**:

- **Paths & sandbox** — default folder for new workspaces and BLXCode Agent file sandbox root.
- **Embedded browser** — default URL for the Browser tab.
- **Category colors** — presets used for Memory category dots and sidebar accents (formerly under a separate Memory settings tab).
- **Confirm before closing a workspace** — when enabled (default), BLXCode asks before closing a workspace from the sidebar ×, context menu, or Terminals tab close path.
- **Terminal naming** — switch terminal title bars from the native `#1`, `#2` slot numbers to friendly **agent names** (Devon, Tom, Mia, …) drawn from an editable, app-wide name pool. Assignment is deterministic from each terminal's stable `slot_id` (the technical identity used for PTY routing, `sessions.json`, and terminal_key never changes), so a terminal keeps its name as siblings come and go. Add / remove / reset-to-defaults entries in the pool from the same section. Any single terminal can be given a **custom name** via **double-click on its header title** or the header **right-click menu (Rename / Reset name)**; the override persists per slot (`slot_name_overrides`, keyed by `slot_id`) and survives restarts.
- **Architecture LLM prose** — reserved for a future optional LLM pass when rebuilding the architecture map; rebuilds today are deterministic and do not call a model.

See [Workspaces](workspaces.md) for File Diff, Git sync, and the architecture map in Memory.

## Themed UI chrome

BLXCode replaces several browser-native controls with theme-aware alternatives:

- **`<select>` popups** — dropdown menus in the Create Workspace wizard, Remote connection picks, and other settings selectors use the active theme's surface and text colors instead of the OS default white background.
- **Confirmation dialogs** — destructive actions (closing a workspace, removing plans, deleting SSH connections) show a themed modal dialog instead of `window.confirm()`. The dialog uses the app's accent/ danger tokens and keyboard navigation (Tab, Enter, Esc).
- **Create Workspace backdrop** — the wizard overlay has a soft top-edge glow (`box-shadow`) that matches the active theme's accent color. This replaces a flat dimmed backdrop.

## See also

- [Agent Harness](agent-harness.md) — core skills, web tools behavior
- [Troubleshooting](troubleshooting.md) — keyring and key errors
