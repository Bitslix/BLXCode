/// Minimal YAML-frontmatter subset used in memory notes.
///
/// Only the metadata used by memory notes is parsed; everything else is ignored.
/// The delimiter is `---` on its own line.

#[derive(Debug, Default, Clone)]
pub struct MemoryFrontmatter {
    pub title: Option<String>,
    pub enabled: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub managed: Option<String>,
    pub kind: Option<String>,
    pub stale: Option<bool>,
    pub git_rev: Option<String>,
    pub source_paths: Option<Vec<String>>,
}

pub fn parse_frontmatter(body: &str) -> (MemoryFrontmatter, String) {
    if !body.starts_with("---") {
        return (MemoryFrontmatter::default(), body.to_owned());
    }
    let rest = &body[3..];
    let end = match rest.find("\n---") {
        Some(i) => i,
        None => return (MemoryFrontmatter::default(), body.to_owned()),
    };
    let raw = &rest[..end];
    let body_rest = &rest[end + 4..];
    let body_rest = body_rest.strip_prefix('\n').unwrap_or(body_rest);

    let mut fm = MemoryFrontmatter::default();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        if colon == 0 {
            continue;
        }
        let key = trimmed[..colon].trim();
        let value = trimmed[colon + 1..].trim();
        match key {
            "title" => {
                fm.title = Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned());
            }
            "enabled" => {
                fm.enabled = Some(value == "true");
            }
            "tags" => {
                fm.tags = parse_inline_array(value);
            }
            "managed" => {
                fm.managed = Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned());
            }
            "kind" => {
                fm.kind = Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned());
            }
            "stale" => {
                fm.stale = Some(value == "true");
            }
            "git_rev" => {
                fm.git_rev = Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned());
            }
            "source_paths" => {
                fm.source_paths = parse_inline_array(value);
            }
            _ => {}
        }
    }
    (fm, body_rest.to_owned())
}

pub fn serialize_frontmatter(fm: &MemoryFrontmatter, body: &str) -> String {
    let mut lines = vec!["---".to_owned()];
    if let Some(t) = &fm.title {
        lines.push(format!("title: {t}"));
    }
    if let Some(e) = fm.enabled {
        lines.push(format!("enabled: {e}"));
    }
    if let Some(tags) = &fm.tags {
        lines.push(format!("tags: {}", render_inline_array(tags)));
    }
    if let Some(managed) = &fm.managed {
        lines.push(format!("managed: {managed}"));
    }
    if let Some(kind) = &fm.kind {
        lines.push(format!("kind: {kind}"));
    }
    if let Some(stale) = fm.stale {
        lines.push(format!("stale: {stale}"));
    }
    if let Some(git_rev) = &fm.git_rev {
        lines.push(format!("git_rev: {git_rev}"));
    }
    if let Some(source_paths) = &fm.source_paths {
        lines.push(format!(
            "source_paths: {}",
            render_inline_array(source_paths)
        ));
    }
    lines.push("---".to_owned());
    format!("{}\n{body}", lines.join("\n"))
}

pub fn strip_frontmatter(body: &str) -> String {
    parse_frontmatter(body).1
}

fn parse_inline_array(value: &str) -> Option<Vec<String>> {
    if !value.starts_with('[') || !value.ends_with(']') {
        return None;
    }
    let inner = &value[1..value.len() - 1];
    Some(
        inner
            .split(',')
            .map(|t| t.trim().trim_matches(|c| c == '"' || c == '\'').to_owned())
            .filter(|t| !t.is_empty())
            .collect(),
    )
}

fn render_inline_array(values: &[String]) -> String {
    let rendered = values
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_fields_round_trip() {
        let input = "---\ntitle: Architecture\nmanaged: static\nstale: false\ngit_rev: abc1234\nsource_paths: [\"src/lib.rs\", \"src/main.rs\"]\ntags: [\"architecture\"]\n---\n# Body\n";
        let (fm, body) = parse_frontmatter(input);
        assert_eq!(fm.managed.as_deref(), Some("static"));
        assert_eq!(fm.stale, Some(false));
        assert_eq!(fm.git_rev.as_deref(), Some("abc1234"));
        assert_eq!(
            fm.source_paths.as_deref(),
            Some(["src/lib.rs".to_string(), "src/main.rs".to_string()].as_slice())
        );
        let rendered = serialize_frontmatter(&fm, &body);
        let (again, _) = parse_frontmatter(&rendered);
        assert_eq!(again.managed, fm.managed);
        assert_eq!(again.stale, fm.stale);
        assert_eq!(again.git_rev, fm.git_rev);
        assert_eq!(again.source_paths, fm.source_paths);
    }
}
