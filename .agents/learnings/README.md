---
title: Learnings
enabled: true
tags: []
---
# Learnings

This folder is a **growing knowledge base of resolved problems** - concrete things you discovered the hard way and want future agents (human or AI) to find.

Each learning is a separate markdown file named `learning-<slug>.md`. Together they form a searchable log of "the thing we now know that we didn't know before."

Keep this file as the overview and index. Store individual learnings in separate Markdown files inside `.agents/learnings/`.

## When to record a learning

- A bug you fixed that wasn't obvious from the symptom
- A workaround for a quirk in a dependency, runtime, or environment
- A migration step that required out-of-band knowledge

## Tips for AI agents

- Search here before debugging - someone may have hit this already.
- When you fix a non-trivial bug, propose a new learning entry.
- Keep one learning per file; cross-link related ones via `[[wikilinks]]`.

## Index

_(Add learnings here as `[[learnings/topic-filename|Short title]]` - one line per topic.)_
