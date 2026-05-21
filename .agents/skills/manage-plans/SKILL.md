---
name: manage-plans
description: Create, update, and maintain BLXCode workspace plans under `.agents/plans/`. Use when Codex needs to add a new implementation plan, refine an existing plan, update plan task status, keep `PLANS.md` indexed, or explain the repository's Markdown plan format and task syntax to agents.
---

# Manage BLXCode Plans

## Core Rules

- Store durable plans in `.agents/plans/` as Markdown files.
- Keep `.agents/plans/PLANS.md` as the protected index. Do not delete or rename it.
- Use one focused plan file per feature, refactor, investigation, or multi-step implementation.
- Prefer lowercase hyphenated filenames ending in `.md`, for example `kanban-board-view.md`.
- Keep plans agent-actionable: concrete enough to implement, but not bloated with unrelated analysis.
- Update the plan as work changes. A stale plan is worse than no plan.

## Plan File Structure

Use this default structure for new plans:

```markdown
# Short Plan Title

## Summary

Briefly state the goal, current direction, and important constraints.

## Decisions

- Record product or technical decisions that should not be rediscovered.
- Include assumptions when the user has not explicitly decided something.

## Implementation Notes

- Describe the intended approach by subsystem or behavior.
- Mention important files only when needed to avoid ambiguity.
- Include public APIs, storage formats, commands, or UI contracts when they change.

## Tests

- List backend, frontend, integration, and manual checks expected for the work.

## Tasks

- [ ] `stable-task-id` - Concrete task title
```

Small plans may omit `Decisions` or `Implementation Notes` if they would be empty. Keep `Summary`, `Tests`, and `Tasks` whenever the plan is meant to guide implementation.

## Task Syntax

BLXCode parses plan tasks from a `## Tasks` section. `## Todos` is accepted, but prefer `## Tasks`.

Use exactly one task per line:

```markdown
- [ ] `task-id` - Pending task
- [>] `active-task` - In-progress task
- [!] `blocked-task` - Blocked task
- [x] `done-task` - Completed task
- [-] `cancelled-task` - Cancelled task
```

Status markers:

- `[ ]` pending
- `[>]` in progress
- `[!]` blocked
- `[x]` completed
- `[-]` cancelled

Task IDs must be stable, unique within the plan, lowercase where practical, and wrapped in backticks. Do not rename a task ID casually because BLXCode uses it to sync plan-linked tasks.

## Creating A New Plan

1. Check `.agents/plans/PLANS.md` and existing plan filenames to avoid duplicates.
2. Create a focused `.md` file under `.agents/plans/`.
3. Add a clear H1 title and the standard sections above.
4. Add concrete `## Tasks` entries using stable task IDs.
5. Add a row to `.agents/plans/PLANS.md` with status, link, and concise description.

Index row format:

```markdown
| planned | [new-plan.md](new-plan.md) | One-sentence description of the intended work |
```

Use `planned`, `active`, `blocked`, `done`, or `cancelled` in the index status column. Match the style already used in `PLANS.md`.

## Updating Existing Plans

- Update task status markers when work progresses.
- Add tasks when the implementation discovers real new work.
- Remove tasks only when they are invalid or superseded; prefer `[-]` for cancelled work with useful history.
- Keep the index row status aligned with the plan's actual state.
- Preserve unrelated sections and user-authored content.
- Avoid reformatting the whole file unless explicitly asked.

## Quality Bar

- Make each task independently actionable.
- Avoid vague tasks like "fix stuff" or "improve UI".
- Include edge cases and acceptance checks in `Tests`, not as hidden assumptions.
- For UI work, include expected user-visible behavior.
- For storage/API work, include compatibility and migration expectations.
- Do not include implementation details that are merely guesses if the repo can answer them; inspect first.
