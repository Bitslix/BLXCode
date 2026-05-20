# Rules And Skills

BLXCode ships workspace-scoped **rules** (binding constraints) and **skills** (optional how-to guides) that the BLXCode Agent can list, read, and honor. Both live under `.agents/` and are managed from the right panel.

## On-disk layout

```text
<workspace>/.agents/rules/
  index.json              # enabled flags (harness-managed)
  rule-my-convention.md   # one rule per file

<workspace>/.agents/skills/
  index.json              # enabled flags + source metadata
  my-skill/
    SKILL.md              # required for a valid skill folder
```

`index.json` files track which entries are enabled and (for skills) install provenance (`git`, `npm`, `local`, or `agent`). BLXCode writes them atomically (tmp + rename) and prunes orphan entries when reading.

Do not hand-edit `index.json` unless you know what you are doing; use the UI or agent tools instead.

## Rules panel

Open **Rules** from the right workbench rail (`LuShield` icon).

Each card shows:

- Title and summary from the rule file
- **Enabled** / **Disabled** pill
- Toggle, read, and remove controls

Disabled rules are invisible to the agent — the system prompt treats them as if they did not exist.

Active rules are **binding and non-negotiable**; they outrank skill guidance when both apply.

## Skills panel

Open **Skills** from the right workbench rail (`LuPuzzle` icon).

<p align="center">
  <img src="../images/skills-panel.png" alt="Skills panel with install dialog and skill cards showing source badges" />
</p>

Each card shows:

- Skill name and summary
- Source badge: `git`, `npm`, `local`, or `agent`
- **SKILL.md missing** warning when the folder has no top-level `SKILL.md`
- Enable/disable and remove controls

Use **Install skill** to add a skill from:

| Source | Fields |
|--------|--------|
| **Git** | name, repository URL, optional ref |
| **npm** | name, package name, optional version |
| **Local** | name, path to a folder containing `SKILL.md` |

Installs stage into `.install.<name>.tmp/`, verify `SKILL.md` at the top level, then promote or roll back on failure.

## Workspace bootstrap

When a workspace path is set (wizard, switch, or workbench restore), BLXCode runs `skills_rules_bootstrap`:

- Creates `.agents/rules/` and `.agents/skills/` if needed
- Seeds each `index.json` from on-disk content (existing files enter as `enabled: true`)
- Skills without provenance default to `source.kind = "local"`
- Manually disabled entries survive later bootstraps

## Agent turn checklist

Every non-trivial agent turn should follow this order (documented in the system prompt):

1. `rules_list` + `rules_read` on relevant **active** rules
2. `skills_list` + `skills_read` on matching skills when the task warrants it
3. **Resume check** — continue from `task_list` / `activePlanPath` when the user says *continue*, *resume*, *weiter*, *fortsetzen*, and similar
4. Memory, plans, and project context as needed
5. Execute

Trivial conversational turns may skip steps 1–2.

Install or remove skills/rules only when the user explicitly asks.

## Agent tools

| Group | Tools |
|-------|--------|
| Rules | `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove` |
| Skills | `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, `skills_install` |

Use `list_tools` when you need the full catalog and parameter schemas.

## Bootstrap flow

```mermaid
flowchart LR
  Open[Workspace opened]
  Boot[skills_rules_bootstrap]
  Disk[".agents/rules + skills"]
  Idx[index.json]
  Open --> Boot
  Boot --> Disk
  Boot --> Idx
  Idx -.->|enabled flags| Disk
```

## See also

- [Agent Providers](agent-providers.md) — provider settings, context, and `list_tools`
- [Plans](plans.md) — plan files and task sync
- [Getting Started](getting-started.md) — `.agents/` layout overview
