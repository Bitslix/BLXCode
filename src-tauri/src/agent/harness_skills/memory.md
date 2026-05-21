# Workspace Memory

Persist and retrieve notes and learnings scoped to the active workspace.

## Storage roots
- `.agents/memory/` — general notes (category `memory` + user-created subcategories)
- `.agents/learnings/` — durable repo learnings (category `learnings`, API path `learnings/<file>.md`)
- Subdirectories of `.agents/memory/` become their own categories (e.g. `projects/setup.md` → category `projects`).

## Note CRUD (server-side)

### `memory_list`
Lists every note (up to 200) with size and modified time. Cheap overview call.

### `memory_read { path }`
Reads one note. `path` is the API path ending in `.md` (e.g. `projects/setup.md` or `learnings/api-notes.md`).

### `memory_search { query }`
Full-text search; returns up to 50 hits with paths and snippets. Use this before `memory_list` for targeted lookups.

### `memory_create { path, content? }`
Creates a **new** note (32 KiB max). Path must end in `.md` and must not already exist.

### `memory_write { path, content }`
Overwrites an existing note. Prefer this over `memory_create` to avoid near-duplicates.

### `memory_delete { path }`
Deletes one note permanently.

### `memory_rename { oldPath, newPath, rewriteLinks? }`
Renames or moves a note. Cross-root (`memory` ↔ `learnings`) is rejected. `rewriteLinks` defaults to `true` (updates `[[wikilinks]]` in other notes).

### `memory_graph`
Returns graph nodes/edges/tags clustered by category. Use for a high-level overview of the knowledge base.

### `memory_backlinks { path }`
Returns notes that link to the given path via `[[wikilinks]]`.

### `memory_list_categories`
Lists every category present in the workspace (built-in + user-created folders).

### `memory_create_category { name }`
Creates an empty category (subfolder under `.agents/memory/`). Use sparingly — prefer creating the first note with a `<category>/<note>.md` path instead.

## Category UI & agent context (client-side)

### `memory_category_list`
Returns label/color/sidebar/graph flags for every visible category.

### `memory_category_update { category, label?, color?, showInSidebar?, showInGraph? }`
Updates display settings for a category. `color` as `#rrggbb`.

### `memory_context_list`
Lists items currently attached to BLXCode Agent context.

### `memory_context_attach { kind, path?, label? }`
Attaches a memory item. `kind` values: `memory_category`, `learning_category`, `memory_note`, `learning_note` (notes require `path`).

### `memory_context_detach { id }`
Removes an attached context item by id.

### `image_context_list`
Lists images attached to the active Agent context.

### `image_context_detach { id }`
Removes an attached image by id.

## When to read / write

**Read when:** the question is about this repo's conventions, architecture, prior decisions, or pitfalls. Start with `memory_search` (focused query), then `memory_read` on the 1–3 best paths.

**Write when:** a durable convention, decision, API contract, migration step, or non-obvious pitfall emerged from the work.

**Skip when:** trivial questions, single-line fixes, or topics answered entirely from the current message and one file read.

Use `[[wikilinks]]` when linking related notes. Keep notes concise and free of secrets.
