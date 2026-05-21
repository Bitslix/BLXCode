# Plan: Leptos 0.7 → 0.8 + leptos_icons/icondata 0.5 → 0.7

## Status: IN PROGRESS — Cargo.toml already bumped, code not yet fixed

## Context

Upgrading the three coupled frontend deps:
- `leptos` 0.7.8 → 0.8.19
- `leptos_icons` 0.5 → 0.7.1 (requires leptos 0.8)
- `icondata` 0.5 → 0.7 (paired with leptos_icons)

Research confirmed **most leptos API is unchanged** between 0.7 and 0.8. The only true breaking change is in **leptos_icons**: the `icon` prop changed from a bare `icondata_core::Icon` value to `Signal<icondata_core::Icon>`.

---

## Breaking Changes

| Change | Scope | Fix |
|---|---|---|
| `leptos_icons::Icon` prop `icon:` changed from `icondata_core::Icon` to `Signal<icondata_core::Icon>` | 20 files | Wrap every `icon={icondata::LuX}` → `icon=Signal::derive(\|\| icondata::LuX)` |
| leptos prelude now also exports `write::*` (additive) | Possible name shadowing | Fix any name conflicts from compile errors |
| reactive_graph 0.1 → 0.2, tachys 0.1 → 0.2 | Internal | No code changes expected |

---

## Fix Pattern

```rust
// Before (leptos_icons 0.5):
<LxIcon icon={icondata::LuTerminal} width="16" height="16" />

// After (leptos_icons 0.7):
<LxIcon icon=Signal::derive(|| icondata::LuTerminal) width="16" height="16" />
```

`Signal` is already in scope via `leptos::prelude::*` — no extra import needed.

---

## Files to Update (20 files — all `<LxIcon icon={...}>` call sites)

All in `src/workbench/`:
1. `agent_context_handoff.rs`
2. `agent_panel/context_list.rs`
3. `agent_panel/mod.rs`
4. `agent_panel/timeline.rs` ← also has `fn -> icondata::Icon` return type — leave that alone
5. `agent_panel/voice_orb/mod.rs`
6. `create_workspace_wizard.rs`
7. `git_graph/mod.rs`
8. `harness_image_pane/mod.rs`
9. `harness_ui.rs`
10. `harness_voice_pane/mod.rs`
11. `memory_graph/mod.rs`
12. `memory_panel.rs`
13. `model_picker/mod.rs`
14. `plans_panel/mod.rs`
15. `project_explorer/mod.rs`
16. `right_panel.rs`
17. `skills_rules_panel/rules_tab.rs`
18. `skills_rules_panel/skills_tab.rs`
19. `terminal_cell.rs`
20. `workspace_panel.rs`

---

## Execution Steps

1. ~~Bump `leptos = "0.8"`, `leptos_icons = "0.7"`, `icondata = "0.7"` in `Cargo.toml`~~ ✅ Done
2. Run `cargo check -p blxcode-ui --target wasm32-unknown-unknown` — collect all errors
3. For each error: wrap `icondata::Lu*` in `Signal::derive(|| ...)`
4. Re-run `cargo check` — fix any residual errors
5. Final: `cargo check -p blxcode` + `cargo check -p blxcode-ui --target wasm32-unknown-unknown`

---

## APIs Confirmed Unchanged (no edits needed)

- `window_event_listener_untyped` — same (11 files safe)
- `mount_to_body()` — same
- `Effect::new()` — same (19 files safe)
- `spawn_local` at `leptos::task::spawn_local` — same (27 files safe)
- `on_cleanup` — same (14 files safe)
- `Callback<In, Out>` / `Callable` — same (8 files safe)
- `signal()`, `RwSignal::new()`, `.get()`, `.set()` — same
- `Signal::derive()`, `Memo::new()` — same
- `provide_context` / `expect_context` — same
- `NodeRef::<html::Div>`, `NodeRef::<html::Audio>` — same
- `Show`, `For` in `view!` macro — same
