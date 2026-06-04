# Provider-Grouped Model Picker With Logos

## Summary

Add provider logos and provider-grouped accordion behavior to the Agent Composer model picker. The picker should show all agent providers (`OpenRouter`, `OpenAI`, `Anthropic`) as collapsible groups, with exactly one group open at a time. Selecting a model from another provider saves both the provider and model.

OpenRouter model/provider APIs do not provide logo assets, so BLXCode should use local `public/brand-icons/*.svg` assets with a fallback badge for unknown model owners.

## Decisions

- Scope v1 to the Agent Composer model picker shown above the input bar.
- Show all providers in the Composer picker, not only the currently configured provider.
- Keep exactly one provider group expanded at a time.
- Default expanded group is the active configured provider.
- Use local provider/owner logo mapping; do not fetch remote logos.
- Keep existing localStorage favorites behavior and make favorites work across all provider groups.
- Continue using existing `agent_provider_models` and `agent_settings_save` commands; do not add a new backend aggregate endpoint for v1.

## Implementation Notes

- Load and cache model lists per provider in the Composer when the picker opens. Fetch `OpenRouter`, `OpenAI`, and `Anthropic` via the existing provider models command.
- Track `open_provider_group` in Composer state. Switching groups closes the previous group.
- Selecting a model persists the selected provider plus model id through the existing provider settings save path, preserving thinking level, tool loop limit, auto-compact settings, orb mode, and nickname.
- Render provider group headers with local logos:
  - Provider logos: `openrouter.svg`, `openai.svg`, `anthropic.svg`.
  - OpenRouter model owner logos: infer owner from model id prefix before `/`; map common owners such as `openai`, `anthropic`, `google`, `mistral`, `x-ai`/`xai`, `amazon`.
  - Unknown owner: use a subtle initials/logo fallback.
- Preserve current picker affordances: search, active row pinned at top of its provider group, favorite star, model detail line, row select closes picker, outside click/Escape close popovers.
- Search should filter across all groups. Groups with no matches are hidden. If the currently open group has no matches, open the first matching group; clearing search restores the active provider group unless the user manually switched groups.

## Tests

- Unit-test provider/owner slug extraction and logo fallback helpers.
- Unit-test model grouping/filtering so only groups with matching models render during search.
- Manual Composer checks:
  - Open picker with each configured provider and confirm the matching group is expanded.
  - Switch provider by selecting a model in another group and confirm settings/model label update.
  - Confirm only one provider group is open at a time.
  - Confirm favorite stars persist across reloads and across provider groups.
  - Confirm OpenRouter owner logos/fallback badges render without broken images.
  - Confirm provider model fallback/cache behavior remains unchanged when a direct provider API key is missing.

## Tasks

- [x] `provider-model-state` - Add per-provider model loading/cache state in the Composer picker.
- [x] `provider-accordion-ui` - Render provider groups as a one-open accordion with active-provider default.
- [x] `provider-logo-map` - Add local provider/model-owner logo mapping with fallback badges.
- [ ] `cross-provider-select` - Persist provider and model when selecting from another provider group.
- [ ] `search-favorites` - Keep search, active-row pinning, and favorites working across provider groups.
- [ ] `tests-smoke` - Add helper tests and run manual Composer smoke checks.
