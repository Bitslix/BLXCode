# Contributing

Thank you for helping improve BLXCode. The project is young, so clear fixes, docs, small refactors, and careful issue reports all matter.

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

## Pull Request Checklist

- The change is scoped and easy to review.
- New Tauri commands are registered and wrapped.
- User-facing errors are clear.
- Path and workspace operations are sandboxed where relevant.
- Docs are updated for changed behavior.
- Relevant checks were run or explicitly noted.
