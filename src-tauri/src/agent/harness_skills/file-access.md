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

### `workspace_file_write { path, content, create? }`
Create or overwrite a UTF-8 text file under the workspace root. Path is relative to the workspace. Protected folders are rejected.

### `workspace_file_delete { path }`
Delete a file or directory under the workspace root. Directories are removed recursively. Protected folders are rejected.

### `workspace_dir_create { path }`
Create a directory under the workspace root, including missing parents.

### `workspace_entry_rename { oldPath, newPath }`
Rename or move a file or directory under the workspace root.

## Sandbox rules
- All paths are relative to the workspace root. Never use `..` or absolute paths.
- The tools enforce the boundary; path-escape attempts are rejected.
- Write/delete/rename tools are gated by the active Agent Chat mode. In `Ask Edits`, the user approves first; in `Plan`, they are blocked; in `Allow all`, they run directly.
