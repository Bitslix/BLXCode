# Contributing to BLXCode

Thank you for helping improve BLXCode. Clear fixes, documentation, small refactors, and careful issue reports all matter.

## Where to start

| Goal | Channel |
|------|---------|
| Report a bug | [GitHub Issues → Bug report](https://github.com/Bitslix/BLXCode/issues/new?template=bug_report.yml) |
| Request a feature | [GitHub Issues → Feature request](https://github.com/Bitslix/BLXCode/issues/new?template=feature_request.yml) or [Discussions → Ideas](https://github.com/Bitslix/BLXCode/discussions/categories/ideas) |
| Ask a question | [Discussions → Q&A](https://github.com/Bitslix/BLXCode/discussions/categories/q-a) |
| Submit code | Pull request against `main` |

**Full contributor guide:** [docs/developer/contributing.md](docs/developer/contributing.md)

## Development setup

```bash
./scripts/setup/setup-linux.sh    # or setup-macos.sh / setup-windows.ps1
cargo tauri dev
```

See [Developer Setup](docs/developer/setup.md) for prerequisites and verification commands.

## Project rules

Read `.agents/rules/` before making changes:

- Keep modules focused — avoid monolithic files.
- Prefer reusable components with clear boundaries.
- Separate Leptos UI, Tauri commands/state, pure logic, and IO.

## Code conventions

- Register Tauri commands in `src-tauri/src/lib.rs`; add frontend wrappers in `src/tauri_bridge.rs`.
- Keep backend path validation on the backend.
- Mirror agent protocol changes in `src/agent_wire.rs`.
- Add `I18nKey` entries to **all** locale files when UI strings change.
- Use theme tokens (`var(--*)`) for styling — see `.agents/rules/rule-theme-tokens.md`.

## Checks before a pull request

```bash
cargo check -p blxcode
cargo check -p blxcode-ui --target wasm32-unknown-unknown
cargo test --workspace
trunk build
```

If you cannot run a check, say so in the PR description.

## Documentation

Update docs when behavior, configuration, file formats, Tauri commands, or provider flows change. Source of truth is `docs/`; the [GitHub Wiki](https://github.com/Bitslix/BLXCode/wiki) syncs from there on pushes to `main`.

## Pull request checklist

- [ ] Scoped, reviewable change
- [ ] New Tauri commands registered and wrapped
- [ ] User-facing errors are clear
- [ ] Workspace/path operations stay sandboxed where relevant
- [ ] Docs updated for user-visible changes
- [ ] Relevant checks run (or noted in the PR)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
