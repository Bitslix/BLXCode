# Memory Architecture Map

Use this skill when a task asks where code lives, how modules relate, how to start a refactor, or how to orient in an unfamiliar workspace.

## What It Is

BLXCode keeps a harness-generated architecture map in workspace memory:

- `ARCHITECTURE.md` is the curated index. Its generated block is a **Unit / Kind / Root / Map** table refreshed by the harness; its Manual section is safe for prose.
- `architecture/modules/*.md` are static per-unit module skeletons. The indexer is multi-language: a plugin registry detects Rust (`Cargo.toml`), Node/TypeScript/JavaScript (`package.json`), Python (`pyproject.toml`/`setup.py`), CMake (`CMakeLists.txt`), Go (`go.mod`), Zig (`build.zig`), and Jai (`.jai` sources) units; falls back to a **Make** map for plain `Makefile`/`GNUmakefile` projects (C/C++ without CMake); and finally to a **Generic** whole-tree map when no manifest is recognized. Generic and Make maps label the detected source languages by extension, so any language without a dedicated indexer (Go, Ada, OCaml, Haskell, Zig, pure JavaScript, …) is still covered. Note files are named `<kind>-<name>.md` (e.g. `rust-blxcode`, `node-blxcode-eb`) and carry a `kind:` frontmatter field. A rebuild never fails just because a manifest is missing.
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
