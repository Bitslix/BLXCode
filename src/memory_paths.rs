//! Workspace memory paths (relative to `.blxcode/memory/`), shared by UI + click routing.

/// Normalizes link target / filename fragment to API shape (`foo.md`, `dir/bar.md`).
#[must_use]
pub fn slug_to_filename(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "untitled.md".into();
    }
    let base = if trimmed.contains('/') || trimmed.contains('\\') {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_owned()
    };
    if base.to_ascii_lowercase().ends_with(".md") {
        base
    } else {
        format!("{base}.md")
    }
}

/// Rejects empty / absolute / `..` segments. Returns a path relative to `.blxcode/memory/`.
#[must_use]
pub fn sanitize_memory_relative_path(raw: &str) -> Option<String> {
    let mut s = raw.trim().replace('\\', "/");
    while s.starts_with('/') {
        s.remove(0);
    }
    if s.is_empty() || s.contains("..") {
        return None;
    }
    Some(s)
}
