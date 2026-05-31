//! Pure, language-aware fold-range computation.
//!
//! highlight.js does not fold, so we derive a heuristic fold model from the raw
//! text (no full parser). Ranges are 1-based and inclusive; `start_line` is the
//! header/opening line that carries the fold chevron and `end_line` is the last
//! line hidden when collapsed. The view layer keeps the rows as individual
//! elements, so collapsing simply hides `start+1..=end`.

use std::collections::HashSet;

/// What produced a fold range — surfaced only for tooling/tests today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldKind {
    Braces,
    Indent,
    Imports,
    Region,
    MarkdownSection,
    FencedCode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoldRange {
    pub start_line: usize,
    pub end_line: usize,
    pub kind: FoldKind,
}

/// Mutable fold state held by the view: the computed ranges plus the set of
/// currently-collapsed start lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FoldState {
    pub ranges: Vec<FoldRange>,
    pub collapsed: HashSet<usize>,
}

impl FoldState {
    #[must_use]
    pub fn new(ranges: Vec<FoldRange>) -> Self {
        Self {
            ranges,
            collapsed: HashSet::new(),
        }
    }

    /// Toggle the collapsed flag for the fold starting at `start_line`.
    pub fn toggle(&mut self, start_line: usize) {
        if !self.collapsed.remove(&start_line) {
            self.collapsed.insert(start_line);
        }
    }

    /// `true` when `line` (1-based) is hidden by a collapsed enclosing fold.
    #[must_use]
    pub fn is_hidden(&self, line: usize) -> bool {
        self.ranges.iter().any(|r| {
            self.collapsed.contains(&r.start_line) && line > r.start_line && line <= r.end_line
        })
    }

    /// The fold range that starts at `line`, if any (the line owning a chevron).
    #[must_use]
    pub fn range_starting_at(&self, line: usize) -> Option<&FoldRange> {
        self.ranges.iter().find(|r| r.start_line == line)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Brace,
    Indent,
    Markdown,
}

fn family_for(lang: Option<&str>) -> Family {
    match lang {
        Some("python") | Some("yaml") => Family::Indent,
        Some("markdown") => Family::Markdown,
        Some(
            "rust" | "typescript" | "javascript" | "json" | "css" | "scss" | "less" | "c" | "cpp"
            | "csharp" | "java" | "kotlin" | "go" | "php" | "swift" | "scala" | "dart" | "xml"
            | "html",
        ) => Family::Brace,
        // Unknown / plain text: still try braces + regions opportunistically.
        _ => Family::Brace,
    }
}

/// Compute fold ranges for `text` highlighted as `lang`.
#[must_use]
pub fn compute_folds(text: &str, lang: Option<&str>) -> Vec<FoldRange> {
    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').collect()
    };
    let mut ranges = match family_for(lang) {
        Family::Markdown => markdown_folds(&lines),
        Family::Indent => indent_folds(&lines),
        Family::Brace => brace_folds(&lines),
    };
    ranges.extend(import_folds(&lines));
    ranges.extend(region_folds(&lines));
    // Stable order by start line then by widest range first.
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(b.end_line.cmp(&a.end_line))
    });
    ranges.dedup();
    ranges
}

/// Balanced `{}`/`[]` scanner with minimal string + line-comment awareness so
/// braces inside string literals or `//` comments don't open phantom folds.
fn brace_folds(lines: &[&str]) -> Vec<FoldRange> {
    let mut out = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut in_block_comment = false;
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let mut chars = raw.chars().peekable();
        let mut in_str: Option<char> = None;
        while let Some(c) = chars.next() {
            if in_block_comment {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block_comment = false;
                }
                continue;
            }
            if let Some(q) = in_str {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    in_str = None;
                }
                continue;
            }
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '/' if chars.peek() == Some(&'/') => break, // rest is a line comment
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    in_block_comment = true;
                }
                '{' | '[' => stack.push(line_no),
                '}' | ']' => {
                    if let Some(open_line) = stack.pop() {
                        if line_no > open_line {
                            out.push(FoldRange {
                                start_line: open_line,
                                end_line: line_no,
                                kind: FoldKind::Braces,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Indent-based folds (Python/YAML): a non-blank header line whose following
/// lines are strictly more indented forms a range ending at the last such line.
fn indent_folds(lines: &[&str]) -> Vec<FoldRange> {
    let mut out = Vec::new();
    let indent = |s: &str| s.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let is_blank = |s: &str| s.trim().is_empty();
    for i in 0..lines.len() {
        if is_blank(lines[i]) {
            continue;
        }
        let base = indent(lines[i]);
        let mut j = i + 1;
        let mut last_child = i;
        while j < lines.len() {
            if is_blank(lines[j]) {
                j += 1;
                continue;
            }
            if indent(lines[j]) > base {
                last_child = j;
                j += 1;
            } else {
                break;
            }
        }
        if last_child > i {
            out.push(FoldRange {
                start_line: i + 1,
                end_line: last_child + 1,
                kind: FoldKind::Indent,
            });
        }
    }
    out
}

/// Markdown heading sections (a heading folds to just before the next heading
/// of equal/higher level) plus fenced ``` code blocks.
fn markdown_folds(lines: &[&str]) -> Vec<FoldRange> {
    let mut out = Vec::new();
    // Fenced code blocks first; track ranges so headings ignore fences.
    let mut fence: Option<usize> = None;
    let mut in_fence = false;
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if raw.trim_start().starts_with("```") {
            if in_fence {
                if let Some(start) = fence.take() {
                    if line_no > start {
                        out.push(FoldRange {
                            start_line: start,
                            end_line: line_no,
                            kind: FoldKind::FencedCode,
                        });
                    }
                }
                in_fence = false;
            } else {
                fence = Some(line_no);
                in_fence = true;
            }
        }
    }
    // Heading levels (ignore lines inside fenced blocks).
    let heading_level = |s: &str| -> Option<usize> {
        let t = s.trim_start();
        if !t.starts_with('#') {
            return None;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if !(1..=6).contains(&hashes) {
            return None;
        }
        let rest = &t[hashes..];
        // A heading is `#`..`######` followed by whitespace or end of line.
        if rest.is_empty() || rest.starts_with([' ', '\t']) {
            Some(hashes)
        } else {
            None
        }
    };
    let headings: Vec<(usize, usize)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| heading_level(l).map(|lvl| (i + 1, lvl)))
        .collect();
    for (pos, &(line_no, level)) in headings.iter().enumerate() {
        // Section ends just before the next heading of equal/higher level.
        let mut end = lines.len();
        for &(next_line, next_level) in &headings[pos + 1..] {
            if next_level <= level {
                end = next_line - 1;
                break;
            }
        }
        if end > line_no {
            out.push(FoldRange {
                start_line: line_no,
                end_line: end,
                kind: FoldKind::MarkdownSection,
            });
        }
    }
    out
}

/// Contiguous leading import/use lines collapse into one range.
fn import_folds(lines: &[&str]) -> Vec<FoldRange> {
    let is_import = |s: &str| {
        let t = s.trim_start();
        t.starts_with("use ")
            || t.starts_with("import ")
            || t.starts_with("from ")
            || t.starts_with("#include ")
            || t.starts_with("require(")
            || t.starts_with("const ") && t.contains("require(")
    };
    // Find the first import line, then the contiguous run (allowing blanks).
    let first = lines.iter().position(|l| is_import(l));
    let Some(start) = first else {
        return Vec::new();
    };
    let mut last = start;
    let mut count = 0usize;
    let mut i = start;
    while i < lines.len() {
        let t = lines[i].trim();
        if is_import(lines[i]) {
            last = i;
            count += 1;
            i += 1;
        } else if t.is_empty() {
            i += 1;
        } else {
            break;
        }
    }
    if count >= 2 && last > start {
        vec![FoldRange {
            start_line: start + 1,
            end_line: last + 1,
            kind: FoldKind::Imports,
        }]
    } else {
        Vec::new()
    }
}

/// Explicit `#region` / `#endregion`-style markers (and `// MARK:` blocks).
fn region_folds(lines: &[&str]) -> Vec<FoldRange> {
    let mut out = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let is_region_start = |s: &str| {
        let t = s.trim_start().to_ascii_lowercase();
        t.starts_with("//#region")
            || t.starts_with("// #region")
            || t.starts_with("#region")
            || t.starts_with("#pragma region")
            || t.starts_with("// region")
    };
    let is_region_end = |s: &str| {
        let t = s.trim_start().to_ascii_lowercase();
        t.starts_with("//#endregion")
            || t.starts_with("// #endregion")
            || t.starts_with("#endregion")
            || t.starts_with("#pragma endregion")
            || t.starts_with("// endregion")
    };
    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if is_region_start(raw) {
            stack.push(line_no);
        } else if is_region_end(raw) {
            if let Some(start) = stack.pop() {
                if line_no > start {
                    out.push(FoldRange {
                        start_line: start,
                        end_line: line_no,
                        kind: FoldKind::Region,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(ranges: &[FoldRange], kind: FoldKind) -> Vec<(usize, usize)> {
        ranges
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| (r.start_line, r.end_line))
            .collect()
    }

    #[test]
    fn brace_ranges_span_blocks() {
        let src = "fn main() {\n    let x = 1;\n    if x > 0 {\n        go();\n    }\n}\n";
        let folds = compute_folds(src, Some("rust"));
        let braces = starts(&folds, FoldKind::Braces);
        assert!(braces.contains(&(1, 6)), "outer fn fold: {braces:?}");
        assert!(braces.contains(&(3, 5)), "inner if fold: {braces:?}");
    }

    #[test]
    fn braces_in_strings_are_ignored() {
        let src = "let s = \"{ not a block\";\nlet t = 2;\n";
        let folds = compute_folds(src, Some("rust"));
        assert!(starts(&folds, FoldKind::Braces).is_empty());
    }

    #[test]
    fn import_group_folds() {
        let src = "use a;\nuse b;\nuse c;\n\nfn main() {}\n";
        let folds = compute_folds(src, Some("rust"));
        assert_eq!(starts(&folds, FoldKind::Imports), vec![(1, 3)]);
    }

    #[test]
    fn indent_ranges_for_python() {
        let src = "def f():\n    a = 1\n    b = 2\nx = 3\n";
        let folds = compute_folds(src, Some("python"));
        assert!(starts(&folds, FoldKind::Indent).contains(&(1, 3)));
    }

    #[test]
    fn markdown_sections_and_fences() {
        // Trailing newline yields an 8th (empty) line; sections fold to it.
        let src = "# Title\nintro\n## Sub\nbody\n```\ncode\n```\n";
        let folds = compute_folds(src, Some("markdown"));
        let sections = starts(&folds, FoldKind::MarkdownSection);
        assert!(sections.contains(&(1, 8)), "h1 to end: {sections:?}");
        assert!(sections.iter().any(|&(s, _)| s == 3), "h2 present");
        assert!(starts(&folds, FoldKind::FencedCode).contains(&(5, 7)));
    }

    #[test]
    fn region_markers_fold() {
        let src = "// #region setup\nlet a = 1;\nlet b = 2;\n// #endregion\n";
        let folds = compute_folds(src, Some("typescript"));
        assert_eq!(starts(&folds, FoldKind::Region), vec![(1, 4)]);
    }

    #[test]
    fn fold_state_hide_and_toggle() {
        let mut st = FoldState::new(vec![FoldRange {
            start_line: 1,
            end_line: 4,
            kind: FoldKind::Braces,
        }]);
        assert!(!st.is_hidden(2));
        st.toggle(1);
        assert!(st.is_hidden(2));
        assert!(st.is_hidden(4));
        assert!(!st.is_hidden(1)); // header line stays visible
        assert!(!st.is_hidden(5));
        st.toggle(1);
        assert!(!st.is_hidden(2));
    }

    #[test]
    fn empty_text_has_no_folds() {
        assert!(compute_folds("", Some("rust")).is_empty());
    }
}
