# BLXCode Documentation

Welcome to the BLXCode docs. This folder is split into user documentation for running the app and developer documentation for building, modifying, and contributing to it.

## User Docs

- [Getting Started](user/getting-started.md): install prerequisites, run BLXCode, and create your first workspace.
- [Workspaces](user/workspaces.md): workspace creation, terminal grids, panes, browser tabs, and persistence.
- [Agent Providers](user/agent-providers.md): provider selection, API keys, models, thinking levels, and hooks.
- [Memory And Tasks](user/memory-and-tasks.md): project notes, backlinks, graph data, task storage, and agent tools.
- [Troubleshooting](user/troubleshooting.md): common startup, build, browser, keyring, and terminal issues.

## Developer Docs

- [Setup](developer/setup.md): local development environment and verification commands.
- [Architecture](developer/architecture.md): frontend/backend split, important modules, data flow, and storage.
- [Tauri IPC](developer/tauri-ipc.md): command registration, frontend wrappers, and command groups.
- [Internationalization](developer/i18n.md): locales, EULA content, and translation workflow.
- [Contributing](developer/contributing.md): code style, project rules, testing expectations, and pull request checklist.

## Project Principles

BLXCode is a local-first desktop workbench. The app should make it easy to keep workspaces, terminals, memory, tasks, and agent context close together without hiding where data lives. When behavior changes, update the docs beside the code so users and contributors can keep moving without reverse-engineering the app.
