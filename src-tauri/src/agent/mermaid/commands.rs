//! Tauri commands for the Mermaid diagram store and export.
//!
//! - Store CRUD (`mermaid_list_diagrams`, `mermaid_create_diagram`,
//!   `mermaid_delete_diagram`) operate on plan-linked diagrams under
//!   `.agents/plans/<slug>/diagrams/`.
//! - Export commands (`mermaid_export_markdown`, `mermaid_export_pdf`) open a
//!   native "Save As" dialog (tauri-plugin-dialog) and write the chosen file.
//!   The frontend supplies the rendered SVG for PDF; orientation is derived
//!   from the SVG dimensions.

use crate::agent::mermaid::export::{build_markdown, Orientation};
use crate::agent::mermaid::store::{self, DiagramRecord};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn mermaid_list_diagrams(
    workspace_cwd: String,
    slug: String,
) -> Result<Vec<DiagramRecord>, String> {
    store::list_diagrams(&workspace_cwd, &slug)
}

#[tauri::command]
pub fn mermaid_create_diagram(
    workspace_cwd: String,
    slug: String,
    title: String,
    code: String,
    kind: Option<String>,
    task_id: Option<String>,
    id: Option<String>,
) -> Result<DiagramRecord, String> {
    store::create_diagram(
        &workspace_cwd,
        &slug,
        &title,
        &code,
        kind.as_deref().unwrap_or(""),
        task_id,
        id,
        // UI-triggered manual creation: no generating model to record.
        None,
        None,
    )
}

#[tauri::command]
pub fn mermaid_delete_diagram(
    workspace_cwd: String,
    slug: String,
    id: String,
) -> Result<(), String> {
    store::delete_diagram(&workspace_cwd, &slug, &id)
}

/// Export a diagram as a self-contained Markdown file via a native Save dialog.
/// Returns the saved path, or `None` if the user cancelled.
#[tauri::command]
pub fn mermaid_export_markdown(
    app: AppHandle,
    title: String,
    kind: String,
    code: String,
    landscape: bool,
) -> Result<Option<String>, String> {
    let orientation = if landscape {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    };
    let body = build_markdown(&title, &kind, orientation, &code);
    let path = app
        .dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_file_name(format!("{}.md", file_stem(&title)))
        .blocking_save_file();
    write_chosen(path, body.into_bytes())
}

/// Export a diagram as a single-page PDF from its rendered SVG. Orientation is
/// derived from the SVG dimensions. Returns the saved path, or `None` if
/// cancelled.
#[tauri::command]
pub fn mermaid_export_pdf(
    app: AppHandle,
    title: String,
    svg: String,
) -> Result<Option<String>, String> {
    let bytes = svg_to_pdf(&svg)?;
    let path = app
        .dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name(format!("{}.pdf", file_stem(&title)))
        .blocking_save_file();
    write_chosen(path, bytes)
}

fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, String> {
    let options = svg2pdf::usvg::Options::default();
    let tree =
        svg2pdf::usvg::Tree::from_str(svg, &options).map_err(|e| format!("parse svg: {e}"))?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| format!("render pdf: {e}"))
}

fn write_chosen(
    path: Option<tauri_plugin_dialog::FilePath>,
    bytes: Vec<u8>,
) -> Result<Option<String>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let pb = path
        .into_path()
        .map_err(|e| format!("resolve save path: {e}"))?;
    std::fs::write(&pb, bytes).map_err(|e| format!("write file: {e}"))?;
    Ok(Some(pb.to_string_lossy().to_string()))
}

/// Sanitise a title into a safe default file stem.
fn file_stem(title: &str) -> String {
    let s: String = title
        .trim()
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
