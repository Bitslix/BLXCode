# Session Context Window + Compaction

**Status:** done

## Implementation notes (as built)

Phases 1–4 are implemented. One deliberate deviation from the original
design: **no `ConversationCompacted` `AgentEvent`**. Instead
`agent_compact_conversation` runs synchronously and returns the result
(`summary`, `beforeTokens`, `afterTokensEstimate`, `messagesBefore`)
directly — simpler, no protocol/agent_wire/reducer surface. On success the
frontend **resets the visible timeline to a fresh chat** (matching the
backend's now-compacted memory and the user's "start fresh with the
compacted information" request) and shows a status line; the backend keeps
the summary as a synthetic `user`→`assistant` pair so the next turn resumes
from it. Occupancy tracks `ChatUsageStats.last_round_input_tokens` (newest
main-agent `ModelRound` prompt size), reset to the post-compaction estimate.

## Summary

Make the chat session's **context-window budget** visible and manageable:

1. Show how much of the model's context window the current conversation
   occupies (live `used / max · %`) in the chat header.
2. Resolve the **max context size** from the provider's own model metadata
   (OpenRouter `context_length`), with a static fallback table for the direct
   providers (Anthropic / OpenAI) that don't expose it in their model list.
3. Add a **Compact** button in the chat header that summarizes the running
   conversation and restarts the session from the compacted summary (keeping
   continuity, dropping token weight).
4. Add **auto-compact** at ~85 % of the context window — a guarded,
   one-shot-per-threshold trigger that runs the same compaction path before the
   next user turn would overflow.

Related, already landed: the configurable **tool-loop limit** in
Settings → Agent (`AgentProviderSettings.tool_loop_limit`). This plan is the
second half of the same "give the user control over runaway turns" theme.

## Background — what already exists

- Per-round usage is emitted as `AgentEvent::TurnUsage { input_tokens,
  output_tokens, round_index, turn_generation, … }`
  (`src-tauri/src/agent/protocol.rs:191`). One event per provider round.
- The frontend folds these into `ChatUsageStats`
  (`src/workbench/state.rs:188`) via `record_chat_turn_usage`
  (`src/workbench/state.rs:3201`), called from
  `src/workbench/agent_panel/reducer.rs:283` and `timeline.rs:484`.
- The chat header renders `SessionCostChip`
  (`src/workbench/agent_panel/mod.rs:705`) with `total_cost_usd` + `turn_count`.
- The header already hosts action buttons (image-mode, maximize, **reset**) at
  `src/workbench/agent_panel/mod.rs:316-399`; `agent_clear_conversation`
  resets both the backend `state.conversation` and the frontend timeline.
- Model metadata flows through `ProviderModelEntry`
  (`src-tauri/src/agent_settings.rs:47`); OpenRouter `/models` is parsed in
  `fetch_models_live` (`agent_settings.rs:639`) — it currently reads `id`,
  `name`, `description`, `pricing` but **not** `context_length`.
- `pricing.rs` already demonstrates the "static id→value table for direct
  providers" pattern that the context-size fallback should mirror.

### Key correctness note

`ChatUsageStats.total_input_tokens` is a **cumulative sum** across all rounds —
it is *not* the current context occupancy. Each provider round re-sends the
whole conversation, so the **most recent `ModelRound` `input_tokens`** is the
real "tokens currently in the window". The occupancy meter must track the
*latest* round's input tokens (a high-water value reset on clear/compact), not
the running sum used for cost.

## Phase 1 — Resolve max context size from the provider

**Backend (`src-tauri/src/agent_settings.rs`)**

- Add `context_length: Option<u64>` to `ProviderModelEntry` (serde
  `camelCase`, `#[serde(default)]`). Mirror it in the frontend mirror struct
  (`src/tauri_bridge.rs:360`).
- In `fetch_models_live`, parse OpenRouter's `context_length` field into the
  new entry field (add it to the `OpenrouterModel` deserialize struct).
- New module `agent/context_window.rs` with a static fallback table
  `fn fallback_context_length(provider, model_id) -> Option<u64>` covering the
  curated/direct models (e.g. Claude Sonnet/Opus = 200_000, GPT-5 family,
  Gemini 2.5 Pro = 1_000_000…). Pattern-mirror `pricing.rs`. Generic fallback
  returns `None` (UI then shows "—" and skips the meter) — consistent with the
  "informative generic fallback, not a hardcoded few" project rule.
- New resolver `fn resolve_context_length(settings) -> Option<u64>`: prefer the
  cached entry for the active model, else the fallback table.
- New `#[tauri::command] agent_active_context_window()` returning
  `{ provider, modelId, contextLength: Option<u64> }`. Register in `lib.rs`.
  Bridge wrapper in `src/tauri_bridge.rs`.

**Acceptance:** picking a model and refreshing models stores `context_length`;
`agent_active_context_window` returns a number for known models, `None` for
unknown ones.

## Phase 2 — Track live occupancy + render the meter

**Frontend state (`src/workbench/state.rs`)**

- Add `last_round_input_tokens: u64` to `ChatUsageStats` (`#[serde(default)]`,
  reset in `clear_chat_usage`).
- In `record_chat_turn_usage`, when `kind == ModelRound` and `input_tokens`
  is `Some`, set `last_round_input_tokens = max(existing, value)` (or simply
  overwrite with the newest round's value — newest is the most accurate
  occupancy; high-water guards against a late out-of-order event). Decide:
  **overwrite with newest** keyed by `round_index`. Plumb `kind` +
  `round_index` into `record_chat_turn_usage` (currently not passed).

**New component `agent_panel/context_meter/`** (own folder + CSS per
`rule-reusable-components.md`)

- Props: `wb` (for `ChatUsageStats`) + a resolved `context_length` signal.
- Loads `agent_active_context_window()` on mount and when the active model
  changes; stores `context_length: RwSignal<Option<u64>>`.
- Renders next to `SessionCostChip`: `used / max · NN%` with a thin progress
  bar. Color thresholds via theme tokens only (no literals,
  `rule-theme-tokens.md`): normal < 70 %, warn 70–85 %, danger ≥ 85 %.
- When `context_length` is `None`, render just the raw token count (no %/bar).

**i18n:** add keys `AgContextWindowLabel`, `AgContextWindowUnknown`,
`AgContextWindowAria` to `keys.rs` + **all** `locales/*.rs` (compile-time
exhaustiveness — same process used for `AgToolLoopLimit*`).

**Acceptance:** after a turn, the header shows e.g. `112k / 200k · 56%`; the
bar fills and changes color past 70 %/85 %; unknown models show a plain count.

## Phase 3 — Manual Compact button

**Backend — new `agent/compaction.rs`**

- `#[tauri::command] async fn agent_compact_conversation(app, workspaceRoot)`:
  1. Guard on `state.busy()` (reuse the busy/cancel flags in
     `agent/state.rs`); refuse if a turn is running.
  2. Snapshot `state.conversation_snapshot()`. If empty/tiny, no-op.
  3. Run a **single non-tool** provider call (reuse the configured provider +
     key via `load_settings_pub` / `provider_key_pub`) with a summarization
     system prompt: "Compress the conversation into a compact briefing that
     preserves decisions, open tasks, file paths, and current state…". No tools
     passed (so no tool loop).
  4. Replace history: `state.set_conversation(vec![summary_user_or_system
     message])` — store the summary as a single synthetic
     `assistant`/`user` pair (or a `system`-adjacent context block) so the next
     real turn continues from it.
  5. Emit a dedicated event `AgentEvent::ConversationCompacted { summary,
     before_tokens, after_tokens_estimate }` so the timeline can show a
     "Session compacted" divider and the meter can reset occupancy.
- Bump `turn_generation` so any stray in-flight usage is dropped (mirror the
  clear path).
- Register command in `lib.rs`; add bridge wrapper + `agent_wire.rs` event
  variant.

**Frontend**

- Add a **Compact** icon button (e.g. `icondata::LuShrink` / `LuArchive`) in the
  header action row (`agent_panel/mod.rs:316`), disabled while `busy` or
  outside the Tauri shell, with a tooltip.
- On click: call `agent_compact_conversation`; on the `ConversationCompacted`
  event, insert a compaction divider into the `TimelineDoc`, reset
  `last_round_input_tokens` to the post-compaction estimate, keep cost/turn
  totals (cost is historical spend, not occupancy).
- i18n: `AgCompactSession`, `AgCompactSessionAria`, `AgCompactRunning`,
  `AgCompactDoneDivider`.

**Acceptance:** clicking Compact while idle produces a summary, shrinks the
occupancy meter, inserts a "Session compacted" divider, and the next user turn
continues coherently.

## Phase 4 — Auto-compact at ~85 %

**Settings (`agent_settings.rs` + provider pane)**

- Add `auto_compact_enabled: bool` (default `true`) and
  `auto_compact_threshold_pct: u8` (default `85`, clamp 50–95) to
  `AgentProviderSettings` + patch + `AgentProviderSettingsView` mirror.
- Surface in Settings → Agent next to the tool-loop-limit field: a toggle +
  a percent input. i18n keys for label/hint (all locales).

**Frontend trigger (in the context-meter or a small effect in `agent_panel`)**

- Compute `ratio = last_round_input_tokens / context_length` after each turn
  completes (`AgentEvent::Done`), only when `context_length` is known and
  auto-compact is enabled.
- When `ratio >= threshold` **and** not busy **and** not already compacting
  **and** the session wasn't auto-compacted at this threshold since the last
  real growth: call `agent_compact_conversation`.
- Guard against loops with an `auto_compact_armed` flag: disarm right after
  firing, re-arm only once occupancy drops below threshold again (post-compact
  it should). Never auto-fire mid-turn — only between turns.
- Show a subtle status line ("Auto-compacted to stay under the context limit")
  reusing the existing `status_line` signal.

**Acceptance:** with a small artificial context (or a verbose session), once
occupancy crosses the threshold the session auto-compacts exactly once, the
meter drops, and no compaction storm occurs.

## Edge cases / risks

- **Unknown context length:** never auto-compact, never block; show raw count.
- **Provider token accounting differs** (OpenRouter vs Anthropic native usage
  shapes): occupancy is an estimate; label it as such in the aria text.
- **Compaction during cancel/clear:** share the busy/generation guards so a
  reset mid-compaction can't corrupt history.
- **Summary quality:** keep the compaction prompt explicit about preserving
  file paths, task IDs, and unresolved questions; cap summary length so a
  compaction can't itself blow the budget.
- **Subagent usage events** carry `agent_id` — exclude them from the main
  occupancy meter (they don't sit in the main conversation window).

## Out of scope

- Persisting compaction summaries to disk / multi-checkpoint history.
- Token counting client-side (we rely on provider-reported `input_tokens`).
- Per-message pruning UI (compaction is whole-session only for v1).

## Touch list

- `src-tauri/src/agent/protocol.rs` — `ConversationCompacted` event.
- `src-tauri/src/agent_settings.rs` — `context_length`, auto-compact settings,
  resolver + command.
- `src-tauri/src/agent/context_window.rs` *(new)* — fallback table + resolver.
- `src-tauri/src/agent/compaction.rs` *(new)* — summarize + replace command.
- `src-tauri/src/lib.rs` — register new commands.
- `src/tauri_bridge.rs` — mirrors + wrappers.
- `src/agent_wire.rs` — `ConversationCompacted` mirror.
- `src/workbench/state.rs` — `last_round_input_tokens`, recorder changes.
- `src/workbench/agent_panel/mod.rs` — Compact button, auto-compact effect.
- `src/workbench/agent_panel/context_meter/` *(new)* — meter component + CSS.
- `src/workbench/agent_panel/reducer.rs` / `timeline.rs` — pass `kind` +
  `round_index`; handle compaction divider.
- `src/i18n/keys.rs` + `src/i18n/locales/*.rs` — all new keys, every locale.
