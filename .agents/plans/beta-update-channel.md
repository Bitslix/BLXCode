# Beta Channel, Pre-Releases & Background Update Checks

## Summary

Add a Beta update channel to **Settings -> App -> App updates**, backed by GitHub prereleases and SemVer versions like `X.Y.Z-pre.N`. Stable remains default. Beta users receive prereleases plus newer final stable releases. Add an app-global background update checker that runs every 10 minutes using the saved channel, shows discreet statusline progress only while checking, and creates titlebar/native notifications when an update is found.

## Decisions

- Beta channel means "prereleases plus newer stable finals".
- Background checks run every 10 minutes only while the app is open.
- Reuse the existing titlebar notification feed instead of adding a separate update notification system.
- GitHub Releases remains the update CDN/source of truth.
- Existing Tauri updater signing key and artifact format stay unchanged.
- Stable uses the current GitHub `latest.json`; Beta resolves a concrete release tag first because GitHub's latest-release endpoint excludes prereleases.

## Implementation Notes

- Persist update settings in app config, e.g. `<app_config_dir>/app_update_settings.json`, with `channel: "stable" | "beta"` and default `"stable"`. Keep the existing startup auto-check setting, but route all checks through the saved channel.
- Add Tauri IPC:
  - `updater_settings_get() -> { channel }`
  - `updater_settings_save({ channel }) -> { channel }`
  - Extend `updater_check()` to return `channel` and use saved settings.
- Extend updater backend:
  - Stable channel keeps `https://github.com/Bitslix/BLXCode/releases/latest/download/latest.json`.
  - Beta queries GitHub Releases API, ignores drafts, includes prereleases and stable releases, chooses the highest SemVer greater than current, then checks `https://github.com/Bitslix/BLXCode/releases/download/{tag}/latest.json`.
  - Clear pending updates when the channel changes so an update from the previous channel cannot be installed.
- Add app-global background update service:
  - Start after workbench hydration in Tauri shell.
  - Run immediately only if startup auto-check is enabled, then every 10 minutes while enabled.
  - Skip if another check/install is active.
  - Track check source with a mode such as `Manual | Startup | Background` so UI can distinguish behavior.
- Update statusline behavior:
  - Show update status during manual checks and background checks while `Checking`.
  - Hide background `UpToDate` silently.
  - Keep available/install/error states visible for manual flows.
- Update titlebar/notification behavior:
  - When a background check finds a new update, create or update one deduped notification with kind `update`, target `{ "view": "update" }`, and title/body containing version + channel.
  - Send native notification best-effort using the existing notification permission path.
  - Extend titlebar notification click handling so target `update` opens `UpdateDialog`.
  - Optionally add a small update-specific action/icon in the titlebar bell item; clicking it calls the same `updates.open_dialog()`.
- Update Settings UI:
  - Add Stable/Beta segmented control in App updates.
  - Save immediately and run a manual check after channel change.
  - Show current channel near current/available version.
- Extend release/versioning:
  - Accept and sync `X.Y.Z-pre.N` across `src-tauri/tauri.conf.json`, both `Cargo.toml` files, `package.json`, and root `package-lock.json`.
  - Add `--pre-release` to Bash and PowerShell release scripts.
  - Stable + `--pre-release` yields next patch `-pre.1`; prerelease + `--pre-release` increments `N`; `--bump minor|major --pre-release` starts that line at `-pre.1`.
- Extend CI:
  - Detect tags matching `vX.Y.Z-pre.N`.
  - Pass `prerelease: true` to `tauri-apps/tauri-action`.
  - For existing releases, run `gh release edit "$TAG" --prerelease --latest=false`.
  - Continue uploading signed artifacts and canonical `latest.json`.

## Tests

- Backend tests:
  - Update settings default/round-trip/invalid fallback.
  - Stable ignores prereleases.
  - Beta chooses prereleases and newer final stable releases correctly.
  - Pending update clears on channel change.
  - `X.Y.Z-pre.N` parsing and next prerelease computation.
- Frontend checks:
  - Settings channel toggle saves and triggers manual check.
  - Background checks do not open dialogs or show "up to date" noise.
  - Statusline appears only during background checking, then hides.
  - Background update found creates one deduped titlebar item and native notification.
  - Clicking update notification opens `UpdateDialog`.
- Release checks:
  - Bash and PowerShell `--pre-release --dry-run` from stable and prerelease states.
  - Workflow resolves prerelease tags and marks GitHub releases as prerelease/latest=false.
- Full verification:
  - `cargo test`.
  - `cargo check --workspace`.
  - Release scripts dry-run.
  - Manual smoke test with mocked or test GitHub release data.

## Tasks

- [x] `settings-storage` - Add persisted update channel settings and IPC commands
- [x] `channel-aware-updater` - Make update checks stable/beta aware and clear pending updates on channel change
- [x] `background-check-service` - Add 10-minute app-global background update checks
- [x] `update-statusline` - Show discreet statusline progress for manual/background checks
- [x] `update-notifications` - Add deduped update notification and titlebar click target
- [x] `settings-ui` - Add Stable/Beta channel control to Settings -> App -> App updates
- [x] `versioning-scripts` - Add `X.Y.Z-pre.N` parsing and `--pre-release` release script support
- [x] `ci-prerelease` - Mark prerelease CI builds as GitHub prereleases and not latest
- [x] `docs-tests` - Update docs and add backend/frontend/release verification coverage
