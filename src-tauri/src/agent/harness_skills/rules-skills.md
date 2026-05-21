# Rules & Skills

Two customisation layers stored under `<workspace>/.agents/`, each with an `index.json` manifest.

## Rules vs Skills

| | Rules | Skills |
|---|---|---|
| Storage | `.agents/rules/rule-*.md` | `.agents/skills/<name>/SKILL.md` |
| Authority | **Binding** — override your defaults | **Advisory** — apply when relevant |
| Conflict | Rules win over skills | — |
| Naming | `rule-<stem>.md` | `[a-z0-9][a-z0-9-_]{0,40}` |

Only entries with `enabled: true` count. Disabled entries do not exist — never apply, cite, or lobby the user to re-enable them.

## Rules tools (server-side)

### `rules_list`
Returns JSON of every rule with `enabled`, `title`, `summary`, `updatedAt`. Filter by `enabled == true` before applying.

### `rules_read { name }`
Reads the Markdown body of one rule. Read every active rule whose title/summary is plausibly relevant before starting work.

### `rules_write { name, content }`
Creates or overwrites a rule. Name must start with `rule-` and end with `.md`. Only on explicit user request.

### `rules_set_enabled { name, enabled }`
Toggles the manifest flag. Only on explicit user request.

### `rules_remove { name }`
Deletes a rule and cleans its index entry. Confirm with the user before removing.

## Skills tools (server-side)

### `skills_list`
Returns JSON of every skill (core and user) with `enabled`, `title`, `summary`, `source`, `updatedAt`.

### `skills_read { name }`
Reads `SKILL.md` for a user-installed skill, or the embedded content for a core harness skill. Use this to get full guidance for a tool group.

**Core skill names:** `memory`, `plans`, `tasks`, `rules-skills`, `file-access`, `harness`

### `skills_write { name, content }`
Creates or overwrites a skill's `SKILL.md`. Source is set to `agent-created`. Only on explicit user request.

### `skills_set_enabled { name, enabled }`
Toggles the enabled flag. Works for both core and user skills.

### `skills_remove { name }`
Removes a user-installed skill directory and its index entry. Core skills cannot be removed.

### `skills_install { name, source }`
Installs a new skill. `source.kind` is one of:
- `git` — `url` (required) + `ref` (optional branch/tag)
- `npm` — `package` (required) + `version` (optional)
- `local` — `path` (workspace-relative)

The source must contain `SKILL.md` at the top level; otherwise the install is rejected and rolled back. Only call when the user explicitly asks to install something.

## Index files
`.agents/rules/index.json` and `.agents/skills/index.json` are managed by the harness. Do not hand-edit them — use the `*_set_enabled` / `*_remove` / `skills_install` tools instead.
