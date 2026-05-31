//! Pure editability policy for the file editor.
//!
//! Decides whether an opened document is freely editable, read-only by default
//! (policy docs like README/LICENSE that can be promoted via an explicit Edit
//! button), or never editable in-app (binary / too large / protected folder).
//! The backend independently enforces the protected-folder rule on write, so
//! this is the UI-side half of a defense-in-depth pair.

use crate::tauri_bridge::{FileKind, PolicyKind};

/// How the editor may treat an opened document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editability {
    /// Opens editable (code/text) or preview-first with an Edit toggle (plain
    /// markdown). Writes are allowed.
    Editable,
    /// Renders read-only (policy docs) but the title bar offers an Edit button
    /// to promote the open document to the raw editor.
    ReadOnlyByDefault,
    /// No in-UI path to edit: binary, too large/truncated, or protected folder.
    NeverEdit,
}

impl Editability {
    /// `true` when the document can currently be switched into the editor at
    /// all (either directly or via the Edit button).
    #[must_use]
    pub fn is_editable_eventually(self) -> bool {
        matches!(self, Self::Editable | Self::ReadOnlyByDefault)
    }
}

/// Path components that the editor refuses to write to. Mirrors
/// `fs_entries::PROTECTED_COMPONENTS` on the backend; kept in sync by hand
/// because the two crates do not share a types module for this list.
pub const PROTECTED_COMPONENTS: &[&str] = &[
    ".git",
    ".agents",
    ".blxcode",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "coverage",
];

/// `true` when any component of the relative path matches a protected folder.
#[must_use]
pub fn is_protected_rel(rel: &str) -> bool {
    rel.split(['/', '\\'])
        .any(|c| PROTECTED_COMPONENTS.contains(&c))
}

/// Resolve editability from the metadata the preview already computed.
///
/// Resolution order (highest wins): `NeverEdit` (binary / truncated /
/// protected) > `ReadOnlyByDefault` (policy docs) > `Editable`.
#[must_use]
pub fn resolve_editability(
    kind: FileKind,
    policy_kind: Option<PolicyKind>,
    rel_path: &str,
    truncated: bool,
) -> Editability {
    // Only text-like kinds ever reach the editor. Images/videos/binaries are
    // routed to their own viewers and can never be edited.
    if matches!(kind, FileKind::Image | FileKind::Video | FileKind::Binary) {
        return Editability::NeverEdit;
    }
    if truncated || is_protected_rel(rel_path) {
        return Editability::NeverEdit;
    }
    if policy_kind.is_some() {
        return Editability::ReadOnlyByDefault;
    }
    Editability::Editable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_and_text_are_editable() {
        assert_eq!(
            resolve_editability(FileKind::Code, None, "src/main.rs", false),
            Editability::Editable
        );
        assert_eq!(
            resolve_editability(FileKind::Text, None, "notes.txt", false),
            Editability::Editable
        );
        assert_eq!(
            resolve_editability(FileKind::Markdown, None, "doc.md", false),
            Editability::Editable
        );
    }

    #[test]
    fn policy_docs_are_read_only_by_default() {
        assert_eq!(
            resolve_editability(
                FileKind::Markdown,
                Some(PolicyKind::Readme),
                "README.md",
                false
            ),
            Editability::ReadOnlyByDefault
        );
        assert_eq!(
            resolve_editability(
                FileKind::Markdown,
                Some(PolicyKind::License),
                "LICENSE",
                false
            ),
            Editability::ReadOnlyByDefault
        );
    }

    #[test]
    fn binary_truncated_and_protected_are_never_edit() {
        assert_eq!(
            resolve_editability(FileKind::Binary, None, "blob.bin", false),
            Editability::NeverEdit
        );
        assert_eq!(
            resolve_editability(FileKind::Code, None, "big.rs", true),
            Editability::NeverEdit
        );
        assert_eq!(
            resolve_editability(FileKind::Code, None, "node_modules/p/index.js", false),
            Editability::NeverEdit
        );
        // Protected beats policy.
        assert_eq!(
            resolve_editability(
                FileKind::Markdown,
                Some(PolicyKind::Agents),
                ".agents/AGENTS.md",
                false
            ),
            Editability::NeverEdit
        );
    }

    #[test]
    fn protected_matches_full_components_only() {
        assert!(is_protected_rel(".git/config"));
        assert!(is_protected_rel("a/target/x"));
        assert!(!is_protected_rel("src/main.rs"));
        assert!(!is_protected_rel("targets/list.txt"));
    }
}
