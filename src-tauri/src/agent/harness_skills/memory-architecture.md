# Memory Architecture Map

Use this skill when a task asks where code lives, how modules relate, how to start a refactor, or how to orient in an unfamiliar workspace.

## What It Is

BLXCode keeps a harness-generated architecture map in workspace memory:

- `ARCHITECTURE.md` is the curated index. Its generated block is refreshed by the harness; its Manual section is safe for prose.
- `architecture/modules/*.md` are static per-crate module skeletons generated from Rust source layout and Cargo workspace membership.
- `architecture/flows/` is reserved for hand-authored flow notes.

The map is part of memory, so graph, search, wikilinks, and context attach work with existing memory tools.

## Read-First Flow

1. Read `ARCHITECTURE.md`.
2. Read 1-3 relevant `architecture/modules/*.md` notes.
3. Use `memory_search` with architecture terms when the index is too broad.
4. Only then use `workspace_search`, `git_ls_files`, or broad file listing for exact source details.

## Maintenance

- Never create, overwrite, rename, or delete generated files under `architecture/modules/`.
- Regenerate the map with `memory_rebuild_architecture` after large refactors.
- Run `memory_lint_architecture` to detect stale generated notes after HEAD changes.
- Add human narrative to `ARCHITECTURE.md` Manual or `architecture/flows/`, not inside generated blocks.
