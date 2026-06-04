# BLXCode Plugins

BLXCode plugins are local package directories registered in the app data plugin registry. The first supported package category is `runtime`, which contributes project run command detectors for the titlebar Run menu.

Plugins are declarative in this version. BLXCode reads JSON manifests and detector files; it does not execute plugin code.

## Package Layout

Each plugin package must contain `blx-plugin.json` at its package root:

```json
{
  "id": "runtime-example",
  "name": "Example Runtime",
  "version": "1.0.0",
  "description": "Detects example runtime commands.",
  "category": "runtime",
  "capabilities": ["runCommands"],
  "commands": [
    {
      "capability": "runCommands",
      "path": "run-detectors/example.json"
    }
  ]
}
```

Runtime detector files use the same detector schema as built-in runtime plugins:

```json
{
  "detectors": [
    {
      "id": "node-scripts",
      "kind": "packageJsonScripts"
    }
  ]
}
```

Supported detector kinds are:

- `packageJsonScripts`
- `cargo`
- `goModule`
- `cmakeMake`
- `shellScripts`
- `directRuntime`

## Install Source

Settings -> Plugins installs packages from GitHub URLs. The URL can point at a repository or a package directory under a branch path:

```text
https://github.com/owner/repo/tree/main/packages/runtime-example
```

The installer clones the repository to a staging directory, validates `blx-plugin.json`, copies the package directory into the app data plugin folder, and updates the registry. Built-in plugins are registered with source kind `builtIn`; GitHub-installed packages are removable.

## Runtime Commands

The titlebar Run menu loads enabled `runCommands` plugins for the active workspace, scans local or SSH-remote workspace files, and returns matching commands. Selecting a command creates a new visible terminal slot, waits for the PTY to register, then writes the selected shell command into that terminal.

Runtime plugin categories should stay narrow. Use `runtime` for language, package-manager, shell, framework, and build-tool command providers. Reserve future categories such as `agent`, `mcp`, `ui`, and `workflow` for different capability families.
