# File Access

Explore and read files inside the active workspace sandbox.

## Tools

### `list_tools`
Returns the full catalog of every available tool (name, site, description, parameters schema). Call this when you are unsure what tools exist.

### `list_workspace_files { path?, recursive?, maxEntries? }`
Lists files and directories under the workspace root or a relative subdirectory.
- `path` — relative subdirectory (omit to list the root)
- `recursive` — default `false`; set `true` for a deep scan
- `maxEntries` — cap on returned entries

**Pattern:** Always call `list_workspace_files` before reading files when you do not know the exact path. Do not guess directory names.

### `read_workspace_file { path }`
Reads a UTF-8 text file under the workspace root. Output is truncated at 4 000 characters. Path is relative to the workspace root.

**Pattern:** After exploring the tree with `list_workspace_files`, read only the files you actually need. Cite the path you read in your reply.

## Sandbox rules
- All paths are relative to the workspace root. Never use `..` or absolute paths.
- The tools enforce the boundary; path-escape attempts are rejected.
