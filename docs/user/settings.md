# Settings

BLXCode opens settings in a **center workbench tab** (not a modal). The command palette entry **Open Settings** focuses an existing settings tab or creates one per workspace.

## Sidebar categories

| Category | What it configures |
|----------|-------------------|
| **App** | UI language, STT language + push-to-talk, keyboard shortcut mode, notifications, terminal hooks, app updates |
| **Appearance** | Placeholder (themes planned) |
| **API Keys** | All provider secrets in one pane — see below |
| **Workspace** | Default project directory, agent sandbox root, embedded browser URL, **category colors** for Memory |
| **BLXCode Agent** | Text, image, and voice inference — see below |

Legacy saved categories (`Image`, `Voice`, `Memory`) still open the correct pane.

## API Keys

**Settings → API Keys** is the only place to enter provider secrets.

- LLM providers: OpenRouter, Anthropic, OpenAI, and coming-soon rows (Google, Mistral, Grok xAI).
- Media / search: Tavily, Brave, **fal.ai** (image), **Amazon Polly** (AWS voice).
- One **Save** / **Discard** footer for the whole pane; per-row remove marks keys for deletion on save.
- Keys use the OS keyring (`BLXCode` service) with `BLX_*` env fallback when the store is empty; the UI shows **via env** when a fallback is active.

Agent, image, and voice panes show a short status line pointing here — they do not contain password fields.

## BLXCode Agent

**Settings → BLXCode Agent** uses a grid:

| Area | Settings |
|------|----------|
| **Text** | Provider, thinking level, model (`AgentModelPicker`), refresh |
| **Image** | Provider, quality level, model, auto-save |
| **Voice** | Provider (OpenAI / OpenRouter / AWS), STT + TTS models, recording quality, post-STT behavior, voice picks, speak replies |
| **Web Tools** | Tavily / Brave / disabled backend |

One **Save** / **Discard** at the bottom persists text provider + web tools together. Image and voice sections auto-save on change.

Details: [Agent Providers](agent-providers.md), [Image Mode](image.md), [Voice](voice.md).

## Workspace

**Settings → Workspace**:

- **Paths & sandbox** — default folder for new workspaces and BLXCode Agent file sandbox root.
- **Embedded browser** — default URL for the Browser tab.
- **Category colors** — presets used for Memory category dots and sidebar accents (formerly under a separate Memory settings tab).

See [Workspaces](workspaces.md).

## See also

- [Agent Harness](agent-harness.md) — core skills, web tools behavior
- [Troubleshooting](troubleshooting.md) — keyring and key errors
