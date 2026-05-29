---
title: Architecture
enabled: true
tags: ["architecture"]
managed: static
stale: false
git_rev: d366fffff77386626c1d371e043344a023099d41
source_paths: ["src-tauri/src/agent/anthropic.rs", "src-tauri/src/agent/environment.rs", "src-tauri/src/agent/git_agent.rs", "src-tauri/src/agent/mod.rs", "src-tauri/src/agent/openrouter.rs", "src-tauri/src/agent/pricing.rs", "src-tauri/src/agent/project_docs.rs", "src-tauri/src/agent/protocol.rs", "src/agent_wire.rs", "src/app.rs", "src/boot_loading.rs", "src/config/app.config.rs", "src/config/mod.rs", "src/i18n/eula.rs", "src/i18n/keys.rs", "src/i18n/locale.rs"]
---
# Architecture

This index is maintained by BLXCode's architecture map harness.

## Manual

Add curated overview notes here. The generated block below is refreshed by `memory_rebuild_architecture`.

<!-- architecture:static:begin -->
## Generated

| Unit | Kind | Root | Map |
|---|---|---|---|
| `blxcode` | rust | `src-tauri` | [[architecture/modules/rust-blxcode.md|rust-blxcode]] |
| `blxcode-ui` | rust | . | [[architecture/modules/rust-blxcode-ui.md|rust-blxcode-ui]] |
| `blxcode-frontend-js` | node | `frontend-js` | [[architecture/modules/node-blxcode-frontend-js.md|node-blxcode-frontend-js]] |

### Counts

- Units: 3
- Kinds: node, rust
- Top-level modules: 40
- Git revision: `e6499a55910507c4fe85ed436954395a20b7e169`
<!-- architecture:static:end -->
