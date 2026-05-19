# Memory And Tasks

BLXCode keeps project memory and tasks inside the workspace folder so they can travel with the project.

## Memory Storage

General memory notes live here:

```text
<workspace>/.agents/memory/
```

Durable repo learnings (conventions, pitfalls, decisions) live here:

```text
<workspace>/.agents/learnings/
```

In the Memory panel and agent tools, learnings use API paths with a `learnings/` prefix (for example `learnings/my-topic.md`). General notes use paths relative to `.agents/memory/` (for example `notes/idea.md`).

Template notes live under:

```text
<workspace>/.agents/memory/_templates/
```

When you open a workspace, BLXCode creates `.agents/memory/` and `.agents/learnings/` if they are missing. If `.agents/memory/` is empty but legacy `.blxcode/memory/` still has notes, content is copied once into `.agents/memory/` (the legacy folder is left in place).

Paths are sandboxed per root. BLXCode rejects absolute paths, `..` escapes, and non-Markdown files for note operations.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-58-53.png" alt="Memory panel showing workspace note files" />
</p>

## Note Links

Memory supports an Obsidian-style subset:

- `[[Note Name]]`: links to `Note Name.md` by basename.
- `[[folder/Note]]`: links to an explicit relative path.
- `[[learnings/topic|alias]]`: links to a learning note.
- `[[Note Name|alias]]`: uses display alias text while preserving graph linking.
- `#tag`: marks graph metadata.

Existing learnings that use Markdown index links (`[Title](topic.md)`) are upgraded to wikilinks when the workspace is opened so the graph can show connections.

## Graph And Search

The backend can build graph data from notes, backlinks, and tags across both memory and learnings. It can also search notes and return line-level snippets.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-58-47.png" alt="Memory graph showing linked notes in the workspace memory panel" />
</p>

## Agent Memory Pointers

BLXCode can install memory pointer files for agent tools. The current mapping is:

| Agent | Pointer File |
|---|---|
| Claude | `CLAUDE.md` |
| Codex | `AGENTS.md` |
| Gemini | `GEMINI.md` |

Pointers help external coding agents discover BLXCode workspace memory and learnings paths.

## Import And Export

Export writes `memory/` and `learnings/` subdirectories under the destination folder. Import accepts the same layout or a flat tree (imported into `.agents/memory/`).

## Task Storage

Tasks live here:

```text
<workspace>/.blxcode/tasks/index.json
```

Each task includes:

- ID.
- Title and description.
- Status.
- Position.
- Created, updated, and completed timestamps.
- Optional parent task.
- Optional notes.

Supported statuses are:

- `pending`
- `in_progress`
- `blocked`
- `completed`
- `cancelled`

Task writes are serialized through the backend and stored as pretty JSON. The store has a version number so future migrations can detect incompatible formats.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-46-39.png" alt="Agent panel showing workspace task context and task-tool output" />
</p>
