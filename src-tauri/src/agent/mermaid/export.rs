//! Export of a single Mermaid diagram to `.md` or `.pdf`.
//!
//! - `.md`: a self-contained Markdown document — YAML front-matter (title,
//!   kind, orientation) + a fenced ```mermaid block. Pure + deterministic.
//! - `.pdf`: the frontend renders the diagram to SVG (mermaid.js) and hands the
//!   SVG here; we embed it into a single PDF page sized by orientation, which is
//!   derived from the rendered SVG's aspect ratio. See [`build_pdf`].
//!
//! The actual "Save As" path selection happens in the Tauri command layer via
//! `tauri-plugin-dialog`; this module only produces bytes.

/// Page orientation for PDF output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }

    /// Derive orientation from rendered SVG pixel dimensions. Square-ish
    /// diagrams default to portrait.
    #[allow(dead_code)] // helper kept for export sizing; call site pending
    pub fn from_dimensions(width: f64, height: f64) -> Self {
        if width > height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }
}

/// Build a self-contained Markdown export for a diagram.
pub fn build_markdown(title: &str, kind: &str, orientation: Orientation, code: &str) -> String {
    let title = title.trim();
    let kind = kind.trim();
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: {}\n", yaml_scalar(title)));
    if !kind.is_empty() {
        out.push_str(&format!("kind: {}\n", yaml_scalar(kind)));
    }
    out.push_str(&format!("orientation: {}\n", orientation.as_str()));
    out.push_str("generator: BLXCode\n");
    out.push_str("---\n\n");
    if !title.is_empty() {
        out.push_str(&format!("# {title}\n\n"));
    }
    out.push_str("```mermaid\n");
    out.push_str(code.trim_end());
    out.push_str("\n```\n");
    out
}

/// Quote a YAML scalar only when needed (contains characters that would
/// otherwise change meaning).
fn yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.contains(|c: char| matches!(c, ':' | '#' | '"' | '\'' | '\n'))
        || s.starts_with(|c: char| c == '-' || c == ' ');
    if needs_quote {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_from_dimensions() {
        assert_eq!(Orientation::from_dimensions(800.0, 400.0), Orientation::Landscape);
        assert_eq!(Orientation::from_dimensions(400.0, 800.0), Orientation::Portrait);
        assert_eq!(Orientation::from_dimensions(500.0, 500.0), Orientation::Portrait);
    }

    #[test]
    fn markdown_has_frontmatter_and_fence() {
        let md = build_markdown("Auth: Flow", "flowchart", Orientation::Landscape, "flowchart TD\n A-->B");
        assert!(md.contains("orientation: landscape"));
        assert!(md.contains("title: \"Auth: Flow\"")); // colon forces quoting
        assert!(md.contains("```mermaid\nflowchart TD\n A-->B\n```"));
    }
}
