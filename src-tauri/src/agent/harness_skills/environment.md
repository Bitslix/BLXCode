---
name: environment
description: Detect workspace runtime facts such as OS, architecture, shell, git availability, and workspace root before shell or git work.
categorie: workspace
---

# Environment

## `environment_detect {}`

Detect OS, architecture, default shell, git availability, and workspace root.

**Required** before `shell_exec` or any `git_*` tool in the same session.
