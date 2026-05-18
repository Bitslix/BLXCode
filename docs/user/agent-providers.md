# Agent Providers

BLXCode includes an agent panel that can stream turns from remote model providers and execute registered tools. Provider settings are stored locally and can be changed from the app UI.

## Supported Provider Types

- **OpenRouter**: default provider kind. The default model ID is `openai/gpt-5`.
- **Anthropic**: native Anthropic Messages API path.
- **OpenAI-compatible**: OpenAI API-compatible chat/model path.

Model lists are fetched live when possible. If a provider request fails or returns no models, BLXCode falls back to cached or curated model entries.

## API Keys

API keys are saved per provider. BLXCode tries to store them in the OS keyring using service name `BLXCode`.

If keyring access fails on Linux or another platform, BLXCode writes a fallback secret file under the Tauri app config directory. On Unix, the fallback secrets directory is created with private permissions.

The UI only displays masked API key values after save.

## Voice Provider Keys

Voice features reuse the same provider key storage:

- STT with OpenAI uses the saved OpenAI key.
- STT with OpenRouter uses the saved OpenRouter key.
- TTS currently uses OpenAI and therefore needs the saved OpenAI key.

For voice usage details, see [Voice: STT And TTS](voice.md).

## Thinking Levels

The provider settings include a thinking level:

- Off.
- Low.
- Medium.
- High.
- Max.

Different providers interpret reasoning or thinking controls differently. BLXCode maps these settings into provider-specific request payloads where supported.

## Agent Tools

The backend exposes a registry of tools to the model. Current server-side tools include:

- `list_workspace_files`: list files or directories inside the configured workspace root.
- `read_workspace_file`: read UTF-8 text files inside the configured workspace root.
- `memory_list`, `memory_read`, `memory_create`, and related memory tools.
- Task tools for listing, creating, updating, deleting, and reordering workspace tasks.

Workspace file tools are sandboxed to the selected workspace root. Absolute paths and parent-directory escapes are rejected.

## Conversation Flow

The frontend submits a turn through `agent_submit_turn`. The backend starts provider work and queues events. The frontend polls `agent_poll_events` to drain events such as assistant deltas, tool calls, tool results, completion, and errors.

This is a polling design rather than a push/SSE connection from backend to frontend.

Voice-originated turns set a `voice_input` flag. After such a turn completes, BLXCode can synthesize the final assistant text and emit a `voice_ready` event for frontend playback.

## Hooks For External Agents

BLXCode bundles helper scripts under `content/hooks/` for session and title capture:

- Claude.
- Codex.
- Gemini.
- OpenCode.
- Cursor.

The backend can install, inspect, and uninstall supported hooks. Hook scripts use environment values injected into terminal sessions to connect external agent sessions back to the active BLXCode terminal slot.

## Missing Key Behavior

If the selected provider has no configured API key, the agent panel reports the missing key instead of attempting a network request. Add the key in provider settings, save it, and retry the turn.
