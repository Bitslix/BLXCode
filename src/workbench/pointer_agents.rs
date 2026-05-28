//! Shared catalogue of external coding agents that blxcode can advertise
//! its workspace data to (memory pointers, rules pointers, …).
//!
//! Each entry maps a stable agent id (used by the Tauri pointer commands)
//! to the user-visible label, target filename and brand icon. Owned by
//! the workbench so memory and rules panels can share one list.

#[derive(Clone, Copy)]
pub(crate) struct PointerAgent {
    pub id: &'static str,
    pub label: &'static str,
    pub target: &'static str,
    pub icon: &'static str,
}

pub(crate) const POINTER_AGENTS: &[PointerAgent] = &[
    PointerAgent {
        id: "claude",
        label: "Claude",
        target: "CLAUDE.md",
        icon: "/public/brand-icons/anthropic.svg",
    },
    PointerAgent {
        id: "codex",
        label: "Codex",
        target: "AGENTS.md",
        icon: "/public/brand-icons/openai.svg",
    },
    PointerAgent {
        id: "gemini",
        label: "Gemini",
        target: "GEMINI.md",
        icon: "/public/brand-icons/gemini.svg",
    },
    PointerAgent {
        id: "cursor",
        label: "Cursor",
        target: ".cursorrules",
        icon: "/public/brand-icons/cursor.svg",
    },
    PointerAgent {
        id: "opencode",
        label: "OpenCode",
        target: "AGENTS.md",
        icon: "/public/brand-icons/opencode.svg",
    },
];
