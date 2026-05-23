# Contributing

Thank you for helping improve BLXCode. The project is young, so clear fixes, docs, small refactors, and careful issue reports all matter.

**Quick links:** [CONTRIBUTING.md](../../CONTRIBUTING.md) (repository root) · [Developer Setup](setup.md) · [Support](../../SUPPORT.md)

## Before You Change Code

Read the repository rules in `.agents/rules/`:

- Keep modules focused and avoid monolithic files.
- Prefer reusable components and clear component boundaries.
- Keep Leptos UI, Tauri command/state, pure logic, and IO concerns separated.

## Code Style

- Preserve the existing Rust style and module organization.
- Keep `src-tauri/src/lib.rs` focused on Tauri setup and command registration.
- Add frontend invoke wrappers in `src/tauri_bridge.rs`.
- Keep backend path validation on the backend.
- Prefer small modules or submodules for new features.
- Add succinct comments only where the intent is not obvious from the code.

## Frontend Components

For larger UI additions, prefer a component folder with colocated component-specific styling when appropriate. Global tokens and broad app styles can remain in `styles.css`, but feature-specific styles should be easy to find.

## Agent harness changes

When you change agent tools, core skills, subagents, or web settings:

- Update `src-tauri/src/agent/harness_skills/*.md` and `CORE_SKILLS` if adding core documentation.
- Mirror protocol changes in `src/agent_wire.rs`.
- Add `I18nKey` entries to **all** locale files when UI labels change.
- Update [Agent Harness](../developer/agent-harness.md) / [Subagents](../developer/subagents.md) (and matching user docs) when behaviour is user-visible.

## Testing And Checks

Run the narrowest useful check while developing:

```bash
cargo check -p blxcode
cargo check -p blxcode-ui --target wasm32-unknown-unknown
```

Before opening a pull request, run:

```bash
cargo test --workspace
trunk build
```

If you cannot run a check, mention that in the pull request.

## Documentation Expectations

Update docs when a change affects:

- User workflows.
- Configuration.
- File formats.
- Tauri commands or permissions.
- Provider behavior.
- Memory/task storage.
- Development setup.

When adding a new user-facing guide under `docs/user/`, link it from [docs/README.md](../README.md) and [getting-started.md](../user/getting-started.md) (or another hub page with a clear cross-link).

## Publishing docs to GitHub Wiki

User and developer guides under `docs/` are mirrored to the [BLXCode GitHub Wiki](https://github.com/Bitslix/BLXCode/wiki) by CI (`.github/workflows/wiki-sync.yml`) when `main` changes under `docs/**`.

- **Source of truth:** edit Markdown only in this repository (`docs/user/`, `docs/developer/`, `docs/README.md`).
- **Do not edit the wiki in the browser** — the next sync overwrites wiki pages.
- **First-time setup:** enable Wiki under **Settings → Features → Wiki**. If `git clone …BLXCode.wiki.git` fails with “repository not found”, create any one page once in the Wiki tab (for example **Home**), then run **Actions → Wiki Sync → Run workflow** or merge a `docs/**` change to `main`.
- **Local dry-run:** `./scripts/sync_github_wiki.sh --dry-run`
- **Local push** (maintainers): `./scripts/sync_github_wiki.sh` with `git` credentials or `WIKI_SYNC_TOKEN` set.

Wiki page names use `User-*` and `Developer-*` prefixes (for example `docs/user/plans.md` → `User-Plans`). Screenshots stay in `docs/images/` and are linked via `raw.githubusercontent.com`.

## Pull Request Checklist

- The change is scoped and easy to review.
- New Tauri commands are registered and wrapped.
- User-facing errors are clear.
- Path and workspace operations are sandboxed where relevant.
- Docs are updated for changed behavior.
- Relevant checks were run or explicitly noted.
