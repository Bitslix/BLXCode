//! `mermaid_create` / `mermaid_create_many` agent-tool logic.
//!
//! Both tools return the same JSON shape so the frontend can render a single
//! diagram inline and 2+ diagrams as an expandable group:
//!
//! ```json
//! { "diagrams": [ { "id", "title", "kind", "code", "taskId", "planSlug", "persisted" } ] }
//! ```
//!
//! When `plan_slug` is supplied the diagram is persisted under that plan's
//! `diagrams/` folder; otherwise it is returned as an ephemeral (non-persisted)
//! diagram for inline display + on-demand export.

use crate::agent::mermaid::store;
use serde::Serialize;
use serde_json::Value;

/// Max Mermaid source size accepted from the model (guards against runaway
/// generations bloating the plan folder).
const MAX_CODE_BYTES: usize = 32 * 1024;
/// Hard cap on diagrams created in a single `mermaid_create_many` call.
const MAX_BATCH: usize = 12;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagramOut {
    id: String,
    title: String,
    kind: String,
    code: String,
    task_id: Option<String>,
    plan_slug: Option<String>,
    persisted: bool,
}

#[derive(Serialize)]
struct DiagramsEnvelope {
    diagrams: Vec<DiagramOut>,
}

/// Result of building/persisting a set of diagrams: a JSON string ready for the
/// tool `content`, or an error message.
pub fn run_create(ws: &str, args: &Value) -> Result<String, String> {
    let one = parse_one(args)?;
    let out = build_one(ws, one)?;
    finish(vec![out])
}

pub fn run_create_many(ws: &str, args: &Value) -> Result<String, String> {
    let plan_slug = opt_str(args, "plan_slug");
    let items = args
        .get("diagrams")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "`diagrams` array is required".to_string())?;
    if items.is_empty() {
        return Err("`diagrams` array is empty".into());
    }
    if items.len() > MAX_BATCH {
        return Err(format!("too many diagrams (max {MAX_BATCH})"));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let mut spec = parse_one(item)?;
        // A batch-level plan_slug applies to every entry unless overridden.
        if spec.plan_slug.is_none() {
            spec.plan_slug = plan_slug.clone();
        }
        out.push(build_one(ws, spec)?);
    }
    finish(out)
}

struct Spec {
    title: String,
    code: String,
    kind: String,
    task_id: Option<String>,
    plan_slug: Option<String>,
    id: Option<String>,
}

fn parse_one(v: &Value) -> Result<Spec, String> {
    let title = req_str(v, "title")?;
    let code = req_str(v, "code")?;
    if code.len() > MAX_CODE_BYTES {
        return Err(format!(
            "diagram code exceeds {MAX_CODE_BYTES} byte limit ({} bytes)",
            code.len()
        ));
    }
    Ok(Spec {
        title,
        code,
        kind: opt_str(v, "kind").unwrap_or_default(),
        task_id: opt_str(v, "task_id"),
        plan_slug: opt_str(v, "plan_slug"),
        id: opt_str(v, "id"),
    })
}

fn build_one(ws: &str, spec: Spec) -> Result<DiagramOut, String> {
    match &spec.plan_slug {
        Some(slug) => {
            let rec = store::create_diagram(
                ws,
                slug,
                &spec.title,
                &spec.code,
                &spec.kind,
                spec.task_id.clone(),
                spec.id,
            )?;
            Ok(DiagramOut {
                id: rec.meta.id,
                title: rec.meta.title,
                kind: rec.meta.kind,
                code: rec.code,
                task_id: rec.meta.task_id,
                plan_slug: Some(slug.clone()),
                persisted: true,
            })
        }
        None => Ok(DiagramOut {
            id: spec.id.unwrap_or_else(|| slugify(&spec.title)),
            title: spec.title,
            kind: spec.kind,
            code: spec.code,
            task_id: spec.task_id,
            plan_slug: None,
            persisted: false,
        }),
    }
}

fn finish(diagrams: Vec<DiagramOut>) -> Result<String, String> {
    serde_json::to_string(&DiagramsEnvelope { diagrams })
        .map_err(|e| format!("serialize diagrams: {e}"))
}

fn slugify(title: &str) -> String {
    let s: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "diagram".to_string()
    } else {
        s
    }
}

fn req_str(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("`{key}` is required"))
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ephemeral_when_no_plan_slug() {
        let out = run_create(
            "/nonexistent-ws-ignored",
            &json!({ "title": "Flow", "code": "flowchart TD\n A-->B" }),
        )
        .unwrap();
        assert!(out.contains("\"persisted\":false"));
        assert!(out.contains("\"id\":\"flow\""));
    }

    #[test]
    fn rejects_oversize_code() {
        let big = "x".repeat(MAX_CODE_BYTES + 1);
        let err = run_create("/ws", &json!({ "title": "T", "code": big })).unwrap_err();
        assert!(err.contains("byte limit"));
    }

    #[test]
    fn many_rejects_empty() {
        let err = run_create_many("/ws", &json!({ "diagrams": [] })).unwrap_err();
        assert!(err.contains("empty"));
    }
}
