//! Agent-authored Mermaid diagrams: persistence (`store`), agent-tool logic
//! (`tool`), and `.md` / `.pdf` export (`export`).
//!
//! Plan-/task-linked diagrams are stored under
//! `.agents/plans/<slug>/diagrams/`. Ad-hoc chat diagrams (no `plan_slug`) are
//! returned to the timeline but not persisted; they are materialised only on
//! explicit export.

pub mod commands;
pub mod export;
pub mod store;
pub mod tool;
