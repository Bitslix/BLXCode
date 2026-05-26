/// Minimal YAML-frontmatter subset used in memory notes.
///
/// Only `title`, `enabled`, and `tags` are parsed; everything else is ignored.
/// The delimiter is `---` on its own line.

#[derive(Debug, Default, Clone)]
pub struct MemoryFrontmatter {
    pub title: Option<String>,
    pub enabled: Option<bool>,
    pub tags: Option<Vec<String>>,
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
                if value.starts_with('[') && value.ends_with(']') {
                    let inner = &value[1..value.len() - 1];
                    fm.tags = Some(
                        inner
                            .split(',')
                            .map(|t| t.trim().trim_matches(|c| c == '"' || c == '\'').to_owned())
                            .filter(|t| !t.is_empty())
                            .collect(),
                    );
                }
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
        let rendered = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("tags: [{rendered}]"));
    }
    lines.push("---".to_owned());
    format!("{}\n{body}", lines.join("\n"))
}

pub fn strip_frontmatter(body: &str) -> String {
    parse_frontmatter(body).1
}
