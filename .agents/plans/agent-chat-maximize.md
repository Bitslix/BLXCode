# Agent Chat Section Maximize

**Status:** done (implemented on `feat/agent-chat-maximize`)

## Summary

Icon-Toggle im Agent-Tab-Chat-Header (vor Reset): maximiert die Chat-Sektion, indem der Voice-Hero (`agent-hero`) in eine kompakte Leiste wechselt. Tasks, Context und Compose bleiben sichtbar. Nur innerhalb des Agent-Right-Panel-Tabs.

## Decisions

- Voice minimieren via `agent-hero--compact`, kein Ausblenden von Tasks/Context.
- Lokales `chat_maximized` RwSignal, nicht in `WorkspaceEntry`.
- Feature-Branch: `feat/agent-chat-maximize` von `main`.

## Tests

- Agent-Tab: Toggle links vom Reset; Hero kompakt/voll; andere Tabs unberührt.
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown`

## Tasks

- [x] `git-branch` - Branch von main
- [x] `plan-file` - Plan + PLANS.md Index
- [x] `i18n-keys` - AgChatMaximize / AgChatRestore
- [x] `ui-toggle` - Button + Signal + Hero-Modifier
- [x] `css-hero-compact` - Compact hero + icon button styles
- [x] `manual-verify` - wasm check
