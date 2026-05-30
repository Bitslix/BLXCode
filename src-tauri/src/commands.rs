use crate::agent::{dispatch_user_turn, AgentEngineState, EventEnvelope, UserTurn};
use crate::agent_settings::provider_status_json;
use crate::browser_host::BrowserHost;
use crate::pty_host::{path_nav_exec, PathNavResult, PtyManager};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

const AGENT_IMAGE_MAX_BYTES: u64 = 8 * 1024 * 1024;

#[tauri::command]
pub fn agent_submit_turn(
    app: AppHandle,
    turn: UserTurn,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<(), String> {
    dispatch_user_turn(&app, &agent, turn)
}

#[tauri::command]
pub fn agent_poll_events(
    max: usize,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Vec<EventEnvelope> {
    agent.drain(max.max(1).min(512))
}

#[tauri::command]
pub fn agent_abort(agent: State<'_, Arc<AgentEngineState>>) {
    agent.request_cancel();
}

#[tauri::command]
pub fn agent_clear_conversation(agent: State<'_, Arc<AgentEngineState>>) -> Result<(), String> {
    if agent.busy() {
        return Err("Agent ist noch beschäftigt. Bitte zuerst abbrechen oder warten.".into());
    }
    agent.clear_conversation();
    Ok(())
}

#[tauri::command]
pub fn agent_provider_status() -> serde_json::Value {
    provider_status_json()
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentImageFilePayload {
    pub label: String,
    pub mime: String,
    pub bytes_b64: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn agent_read_image_file(path: String) -> Result<AgentImageFilePayload, String> {
    let path = std::path::PathBuf::from(path);
    let meta = std::fs::metadata(&path).map_err(|e| format!("image metadata: {e}"))?;
    if !meta.is_file() {
        return Err("dropped item is not a file".into());
    }
    if meta.len() > AGENT_IMAGE_MAX_BYTES {
        return Err(format!(
            "image exceeds {} MiB limit",
            AGENT_IMAGE_MAX_BYTES / 1024 / 1024
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("read image: {e}"))?;
    let mime = detect_supported_image_mime(&bytes, &path)
        .ok_or_else(|| "only PNG, JPEG, WebP, and GIF images can be attached".to_string())?;
    let label = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Dropped image")
        .to_owned();
    Ok(AgentImageFilePayload {
        label,
        mime: mime.to_owned(),
        bytes_b64: BASE64.encode(bytes),
        size_bytes: meta.len(),
    })
}

fn detect_supported_image_mime<'a>(bytes: &[u8], path: &std::path::Path) -> Option<&'a str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        Some("gif") => Some("image/gif"),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextImageInput {
    pub id: String,
    pub label: String,
    pub mime: String,
    pub bytes_b64: String,
    #[serde(default)]
    pub size_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextImageExport {
    pub id: String,
    pub label: String,
    pub mime: String,
    pub size_bytes: u64,
    pub path: String,
    pub filename: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextExportReport {
    pub dir: String,
    pub manifest_path: String,
    pub images: Vec<AgentContextImageExport>,
}

const AGENT_CONTEXT_REL_DIR: &str = ".blxcode/agent-context";
const AGENT_CONTEXT_IMAGES_DIRNAME: &str = "images";
const AGENT_CONTEXT_MANIFEST_NAME: &str = "manifest.json";

/// Sanitize an image id + label into a stable, fs-safe basename suffix.
/// Keeps `[A-Za-z0-9_-]`, replaces everything else with `_`, lowercases.
fn sanitize_image_stem(id: &str, label: &str) -> String {
    let candidate = if !id.trim().is_empty() {
        id.trim()
    } else if !label.trim().is_empty() {
        label.trim()
    } else {
        "image"
    };
    let mut out = String::with_capacity(candidate.len());
    for ch in candidate.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "image".to_owned()
    } else {
        trimmed
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

#[tauri::command]
pub fn agent_export_context_images(
    workspace_cwd: String,
    items: Vec<AgentContextImageInput>,
) -> Result<AgentContextExportReport, String> {
    let workspace_cwd = workspace_cwd.trim();
    if workspace_cwd.is_empty() {
        return Err("workspace_cwd empty".into());
    }
    let ws_root = std::path::PathBuf::from(workspace_cwd);
    if !ws_root.is_dir() {
        return Err(format!("workspace not found: {workspace_cwd}"));
    }
    let base_dir = ws_root.join(AGENT_CONTEXT_REL_DIR);
    let images_dir = base_dir.join(AGENT_CONTEXT_IMAGES_DIRNAME);
    std::fs::create_dir_all(&images_dir).map_err(|e| format!("create context image dir: {e}"))?;

    let mut exports: Vec<AgentContextImageExport> = Vec::with_capacity(items.len());
    for item in items {
        let bytes = BASE64
            .decode(item.bytes_b64.as_bytes())
            .map_err(|e| format!("decode image {}: {e}", item.id))?;
        let stem = sanitize_image_stem(&item.id, &item.label);
        let ext = extension_for_mime(&item.mime);
        let mut filename = format!("{stem}.{ext}");
        // Avoid clobbering an existing file (preserve idempotency for retries).
        let mut suffix = 1;
        while images_dir.join(&filename).exists() {
            suffix += 1;
            filename = format!("{stem}-{suffix}.{ext}");
            if suffix > 999 {
                return Err(format!("too many collisions for {stem}"));
            }
        }
        let abs_path = images_dir.join(&filename);
        std::fs::write(&abs_path, &bytes)
            .map_err(|e| format!("write {}: {e}", abs_path.display()))?;
        exports.push(AgentContextImageExport {
            id: item.id,
            label: item.label,
            mime: item.mime,
            size_bytes: if item.size_bytes > 0 {
                item.size_bytes
            } else {
                bytes.len() as u64
            },
            path: abs_path.to_string_lossy().into_owned(),
            filename,
        });
    }

    let manifest_path = base_dir.join(AGENT_CONTEXT_MANIFEST_NAME);
    let manifest_json = serde_json::json!({
        "version": 1,
        "writtenAt": chrono_iso_now(),
        "imagesDir": images_dir.to_string_lossy(),
        "images": exports
            .iter()
            .map(|i| serde_json::json!({
                "id": i.id,
                "label": i.label,
                "mime": i.mime,
                "sizeBytes": i.size_bytes,
                "path": i.path,
                "filename": i.filename,
            }))
            .collect::<Vec<_>>(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest_json)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    std::fs::write(&manifest_path, manifest_text)
        .map_err(|e| format!("write manifest {}: {e}", manifest_path.display()))?;

    Ok(AgentContextExportReport {
        dir: base_dir.to_string_lossy().into_owned(),
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        images: exports,
    })
}

/// RFC-3339 UTC timestamp without an extra dependency.
fn chrono_iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Lightweight UTC formatter (YYYY-MM-DDTHH:MM:SSZ).
    let days = (secs / 86_400) as i64;
    let mut year = 1970i64;
    let mut remaining = days;
    let is_leap = |y: i64| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    loop {
        let year_days = if is_leap(year) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        year += 1;
    }
    let months = [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    let mut day_of_year = remaining as u32;
    for (i, &dm) in months.iter().enumerate() {
        let dm = if i == 1 && is_leap(year) { 29 } else { dm };
        if day_of_year < dm {
            month = (i as u32) + 1;
            break;
        }
        day_of_year -= dm;
    }
    let day = day_of_year + 1;
    let hour = ((secs / 3600) % 24) as u32;
    let minute = ((secs / 60) % 60) as u32;
    let second = (secs % 60) as u32;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    use super::detect_supported_image_mime;
    use super::{
        agent_export_context_images, sanitize_image_stem, AgentContextImageInput,
        AGENT_CONTEXT_MANIFEST_NAME, AGENT_CONTEXT_REL_DIR,
    };
    use base64::Engine as _;
    use std::path::Path;

    #[test]
    fn detects_supported_image_magic_bytes() {
        assert_eq!(
            detect_supported_image_mime(b"\x89PNG\r\n\x1a\nrest", Path::new("x.bin")),
            Some("image/png")
        );
        assert_eq!(
            detect_supported_image_mime(b"\xff\xd8\xffrest", Path::new("x.bin")),
            Some("image/jpeg")
        );
        assert_eq!(
            detect_supported_image_mime(b"GIF89arest", Path::new("x.bin")),
            Some("image/gif")
        );
        assert_eq!(
            detect_supported_image_mime(b"RIFFxxxxWEBPrest", Path::new("x.bin")),
            Some("image/webp")
        );
    }

    #[test]
    fn rejects_unknown_image_type() {
        assert_eq!(
            detect_supported_image_mime(b"plain text", Path::new("x.txt")),
            None
        );
    }

    #[test]
    fn sanitize_stem_normalizes_unsafe_chars() {
        assert_eq!(sanitize_image_stem("Img:42", ""), "img_42");
        assert_eq!(sanitize_image_stem("", "Hello World!"), "hello_world");
        assert_eq!(sanitize_image_stem("    ", "  "), "image");
        assert_eq!(sanitize_image_stem("safe-id_1", "anything"), "safe-id_1");
    }

    #[test]
    fn export_writes_images_and_manifest() {
        let tmp = tempdir_with_suffix("blx-ctx-export");
        let ws = tmp.join("workspace with space");
        std::fs::create_dir_all(&ws).unwrap();
        let png = b"\x89PNG\r\n\x1a\nfake".to_vec();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let items = vec![AgentContextImageInput {
            id: "img:1".into(),
            label: "Cover".into(),
            mime: "image/png".into(),
            bytes_b64: b64,
            size_bytes: png.len() as u64,
        }];
        let report =
            agent_export_context_images(ws.to_string_lossy().into(), items).expect("export ok");
        let dir = std::path::PathBuf::from(&report.dir);
        assert!(dir.ends_with(AGENT_CONTEXT_REL_DIR));
        let manifest = std::path::PathBuf::from(&report.manifest_path);
        assert!(manifest.ends_with(AGENT_CONTEXT_MANIFEST_NAME));
        assert_eq!(report.images.len(), 1);
        let exported = std::path::PathBuf::from(&report.images[0].path);
        assert!(exported.is_file());
        assert!(exported
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".png"));
        let manifest_body = std::fs::read_to_string(&manifest).unwrap();
        assert!(manifest_body.contains("\"images\""));
        assert!(manifest_body.contains("img:1"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn export_handles_collisions_with_suffix() {
        let tmp = tempdir_with_suffix("blx-ctx-collide");
        std::fs::create_dir_all(&tmp).unwrap();
        let png = b"\x89PNG\r\n\x1a\nx".to_vec();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let items = vec![
            AgentContextImageInput {
                id: "shared".into(),
                label: "A".into(),
                mime: "image/png".into(),
                bytes_b64: b64.clone(),
                size_bytes: 0,
            },
            AgentContextImageInput {
                id: "shared".into(),
                label: "B".into(),
                mime: "image/png".into(),
                bytes_b64: b64,
                size_bytes: 0,
            },
        ];
        let report = agent_export_context_images(tmp.to_string_lossy().into(), items).unwrap();
        assert_eq!(report.images.len(), 2);
        assert_ne!(report.images[0].path, report.images[1].path);
        std::fs::remove_dir_all(&tmp).ok();
    }

    fn tempdir_with_suffix(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!("{tag}-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub call_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[tauri::command]
pub fn agent_submit_tool_result(
    payload: ToolResultPayload,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<(), String> {
    agent.deliver_client_tool_result(&payload.call_id, payload.ok, payload.message, payload.data)
}

/// Returns the user's home directory as a string. Used as the default for
/// the "default project directory" setting that seeds new workspace cwds.
#[tauri::command]
pub fn harness_user_home_dir(app: AppHandle) -> Result<String, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("home dir unavailable: {e}"))?;
    Ok(home.to_string_lossy().into_owned())
}

/// Returns (and idempotently creates) the default sandbox directory under
/// the app data dir. Used as a guaranteed-non-empty fallback workspace
/// root so the agent always has a writable scope to play in.
#[tauri::command]
pub fn harness_ensure_default_sandbox(app: AppHandle) -> Result<String, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir unavailable: {e}"))?;
    let sandbox = base.join("sandbox");
    std::fs::create_dir_all(&sandbox)
        .map_err(|e| format!("create sandbox {}: {e}", sandbox.display()))?;
    Ok(sandbox.to_string_lossy().into_owned())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBoundsPayload {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub visible: bool,
}

/// Dispatches `f` to the Tauri main thread and awaits the result.
/// Required for any WebView2 / wry API call on Windows (add_child, navigate,
/// show, hide, eval, …) — those must run on the UI message-pump thread.
async fn dispatch_on_main<F, T>(app: tauri::AppHandle, f: F) -> Result<T, String>
where
    F: FnOnce(tauri::AppHandle) -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<T, String>>();
    app.clone()
        .run_on_main_thread(move || {
            let _ = tx.send(f(app));
        })
        .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "main thread channel dropped".to_string())?
}

#[tauri::command]
pub async fn browser_sync_bounds(
    app: tauri::AppHandle,
    active_tab_id: Option<u64>,
    rect: BrowserBoundsPayload,
    url_optional: Option<String>,
) -> Result<(), String> {
    dispatch_on_main(app, move |app| {
        app.state::<BrowserHost>()
            .sync_bounds(&app, active_tab_id, rect, url_optional.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn browser_run_js(
    app: tauri::AppHandle,
    tab_id: u64,
    script: String,
) -> Result<(), String> {
    dispatch_on_main(app, move |app| {
        app.state::<BrowserHost>()
            .eval_embedded(&app, tab_id, script)
    })
    .await
}

#[tauri::command]
pub async fn browser_navigate(
    app: tauri::AppHandle,
    tab_id: u64,
    url: String,
) -> Result<(), String> {
    dispatch_on_main(app, move |app| {
        app.state::<BrowserHost>().navigate(&app, tab_id, &url)
    })
    .await
}

#[tauri::command]
pub async fn browser_close_tab(app: tauri::AppHandle, tab_id: u64) -> Result<(), String> {
    dispatch_on_main(app, move |app| {
        app.state::<BrowserHost>().close_tab(&app, tab_id)
    })
    .await
}

#[tauri::command]
pub fn browser_embedding_kind() -> &'static str {
    crate::browser_host::browser_embedding_kind_str()
}

/// HEAD-Probe an die URL, prüft `X-Frame-Options` und `Content-Security-Policy`
/// `frame-ancestors`. Liefert `false`, wenn das Embedding in einem `<iframe>`
/// vom Server verweigert wird (z. B. github.com mit `X-Frame-Options: DENY`).
/// Bei Netzwerkfehlern wird `true` (optimistisch) zurückgegeben — der iframe
/// versucht es selbst und das `error`-Event greift ggf.
#[tauri::command]
pub async fn browser_check_iframable(url: String) -> Result<bool, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    let parsed = url::Url::parse(trimmed).map_err(|e| format!("URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("blxcode/0.1 (iframable-probe)")
        .build()
        .map_err(|e| e.to_string())?;

    // Manche Server (z. B. github.com) antworten auf HEAD anders als auf GET.
    // GET mit Range bringt die Header zurück ohne den ganzen Body zu ziehen.
    let resp = match client
        .get(parsed.clone())
        .header("Range", "bytes=0-0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Ok(true), // Netzwerkfehler — sei optimistisch.
    };

    let headers = resp.headers();

    if let Some(xfo) = headers.get("x-frame-options").and_then(|v| v.to_str().ok()) {
        let v = xfo.trim().to_ascii_lowercase();
        if v == "deny" || v == "sameorigin" || v.starts_with("allow-from") {
            return Ok(false);
        }
    }

    if let Some(csp) = headers
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
    {
        let lc = csp.to_ascii_lowercase();
        if let Some(idx) = lc.find("frame-ancestors") {
            let after = &lc[idx + "frame-ancestors".len()..];
            let directive = after.split(';').next().unwrap_or("").trim();
            // `frame-ancestors 'none'` oder `'self'` (Tauri-Origin ist nicht self) blockt uns.
            if directive.contains("'none'") || directive == "'self'" {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

#[tauri::command]
pub fn path_nav_exec_cmd(base: String, line: String) -> Result<PathNavResult, String> {
    path_nav_exec(base, line)
}

/// Sinnvoller Start-Pfad fürs Verzeichnis-Picker-UI: bevorzugt `$HOME`,
/// fällt auf das Prozess-Arbeitsverzeichnis zurück.
#[tauri::command]
pub fn default_cwd() -> Result<String, String> {
    if let Some(home) = std::env::var_os("HOME") {
        let s = home.to_string_lossy().into_owned();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        let s = userprofile.to_string_lossy().into_owned();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryBrief {
    pub name: String,
    pub hidden: bool,
}

/// Listet alle Unterverzeichnisse von `path` (alphabetisch). Hidden-Flag ist
/// gesetzt für Einträge, die mit `.` beginnen, damit das UI sie ausblenden oder
/// dimmen kann. Files werden ignoriert — dies ist ein Verzeichnis-Browser.
#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirEntryBrief>, String> {
    let trimmed = path.trim();
    let p = if trimmed.is_empty() {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        std::path::PathBuf::from(trimmed)
    };
    let read = std::fs::read_dir(&p).map_err(|e| e.to_string())?;
    let mut out: Vec<DirEntryBrief> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let hidden = name.starts_with('.');
            Some(DirEntryBrief { name, hidden })
        })
        .collect();
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(out)
}

/// Erstellt ein neues Unterverzeichnis `name` innerhalb von `parent`. Liefert
/// den vollständigen Pfad des neu angelegten Ordners zurück. `name` darf keine
/// Pfadtrenner enthalten — Sandboxing für den Directory-Picker.
#[tauri::command]
pub fn create_directory(parent: String, name: String) -> Result<String, String> {
    let parent = parent.trim();
    let name = name.trim();
    if parent.is_empty() {
        return Err("parent darf nicht leer sein".into());
    }
    if name.is_empty() {
        return Err("Name darf nicht leer sein".into());
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("Name enthält ungültige Zeichen".into());
    }
    let parent_path = std::path::PathBuf::from(parent);
    if !parent_path.is_dir() {
        return Err("Elternverzeichnis existiert nicht".into());
    }
    let target = parent_path.join(name);
    std::fs::create_dir(&target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn pty_spawn(
    manager: State<'_, PtyManager>,
    cwd: String,
    env: Option<Vec<(String, String)>>,
) -> Result<u64, String> {
    manager.spawn_session(cwd, env.unwrap_or_default())
}

/// Spawn an ssh terminal for a saved remote connection. The preset and its
/// secrets are resolved server-side (`ssh_remotes::resolve_spec`) so passwords
/// and passphrases never cross into the frontend/JS.
#[tauri::command]
pub fn pty_spawn_remote(
    app: tauri::AppHandle,
    manager: State<'_, PtyManager>,
    connection_id: String,
    terminal_key: String,
    env: Option<Vec<(String, String)>>,
) -> Result<u64, String> {
    let spec = crate::ssh_remotes::resolve_spec(&app, &connection_id, terminal_key)?;
    manager.spawn_remote_session(spec, env.unwrap_or_default())
}

#[tauri::command]
pub fn pty_write(
    manager: State<'_, PtyManager>,
    session_id: u64,
    data_b64: String,
) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| e.to_string())?;
    manager.write(session_id, bytes)
}

#[tauri::command]
pub fn pty_resize(
    manager: State<'_, PtyManager>,
    session_id: u64,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    manager.resize(session_id, rows, cols)
}

#[tauri::command]
pub fn pty_kill(manager: State<'_, PtyManager>, session_id: u64) -> Result<(), String> {
    manager.kill(session_id)
}

#[tauri::command]
pub fn pty_drain(
    manager: State<'_, PtyManager>,
    session_id: u64,
    max_bytes: usize,
) -> Result<String, String> {
    manager.drain_output(session_id, max_bytes)
}

#[tauri::command]
pub async fn pty_drain_wait(
    manager: State<'_, PtyManager>,
    session_id: u64,
    max_bytes: usize,
    timeout_ms: u64,
) -> Result<String, String> {
    manager.drain_output_wait(session_id, max_bytes, timeout_ms)
}

#[tauri::command]
pub fn pty_peek_output(
    manager: State<'_, PtyManager>,
    session_id: u64,
    max_bytes: usize,
) -> Result<String, String> {
    manager.peek_tail(session_id, max_bytes)
}

#[tauri::command]
pub fn git_branch(cwd: String) -> Option<String> {
    crate::git_info::current_branch(std::path::Path::new(&cwd))
}
