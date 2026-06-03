//! Minimal MCP client: JSON-RPC 2.0 over stdio (local process) or HTTP
//! (Streamable HTTP / single JSON response). Implements just the subset the
//! in-app agent needs: `initialize`, `tools/list`, `tools/call`.
//!
//! This is intentionally small rather than pulling the full `rmcp` SDK; it
//! covers the common case of tool discovery + invocation. SSE-style streaming
//! HTTP responses are not parsed beyond the first JSON object.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use super::registry::{McpServer, McpTransport};

const PROTOCOL_VERSION: &str = "2025-06-18";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A tool advertised by an MCP server, already namespaced for the agent loop.
#[derive(Clone, Debug)]
pub struct McpToolSpec {
    /// Namespaced name exposed to the model: `mcp.<server-id>.<tool>`.
    pub qualified_name: String,
    /// Original tool name as the server knows it.
    pub remote_name: String,
    pub description: String,
    /// JSON-Schema object for the tool input.
    pub input_schema: Value,
}

/// A live connection to one MCP server.
pub enum McpClient {
    Stdio(StdioClient),
    Http(HttpClient),
}

impl McpClient {
    /// Connect + `initialize` handshake.
    pub async fn connect(server: &McpServer) -> Result<Self, String> {
        match &server.transport {
            McpTransport::Stdio { command, args, env } => Ok(McpClient::Stdio(
                StdioClient::connect(command, args, env).await?,
            )),
            McpTransport::Http { url, headers } => {
                Ok(McpClient::Http(HttpClient::connect(url, headers).await?))
            }
        }
    }

    pub async fn list_tools(&self, server_id: &str) -> Result<Vec<McpToolSpec>, String> {
        let result = match self {
            McpClient::Stdio(c) => c.request("tools/list", json!({})).await?,
            McpClient::Http(c) => c.request("tools/list", json!({})).await?,
        };
        parse_tool_list(server_id, &result)
    }

    pub async fn call_tool(&self, remote_name: &str, args: &Value) -> Result<String, String> {
        let params = json!({ "name": remote_name, "arguments": args });
        let result = match self {
            McpClient::Stdio(c) => c.request("tools/call", params).await?,
            McpClient::Http(c) => c.request("tools/call", params).await?,
        };
        Ok(stringify_tool_result(&result))
    }
}

fn initialize_params() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "blxcode", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn parse_tool_list(server_id: &str, result: &Value) -> Result<Vec<McpToolSpec>, String> {
    let arr = result
        .get("tools")
        .and_then(|t| t.as_array())
        .ok_or_else(|| "tools/list: missing tools array".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for tool in arr {
        let Some(remote_name) = tool.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let description = tool
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let input_schema = tool
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));
        out.push(McpToolSpec {
            qualified_name: format!("mcp.{server_id}.{remote_name}"),
            remote_name: remote_name.to_string(),
            description,
            input_schema,
        });
    }
    Ok(out)
}

/// Flatten an MCP `tools/call` result into plain text for the agent loop.
fn stringify_tool_result(result: &Value) -> String {
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for block in content {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        parts.push(t.to_string());
                    }
                }
                _ => parts.push(block.to_string()),
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    result.to_string()
}

// ---------------------------------------------------------------------------
// stdio transport
// ---------------------------------------------------------------------------

pub struct StdioClient {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
    next_id: AtomicI64,
}

impl StdioClient {
    async fn connect(
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn mcp server `{command}`: {e}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "mcp server: no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "mcp server: no stdout".to_string())?;
        let client = StdioClient {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicI64::new(1),
        };
        // Handshake.
        client.request("initialize", initialize_params()).await?;
        client.notify("notifications/initialized").await?;
        Ok(client)
    }

    async fn notify(&self, method: &str) -> Result<(), String> {
        let line = format!("{}\n", json!({ "jsonrpc": "2.0", "method": method }));
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("mcp write: {e}"))?;
        stdin.flush().await.map_err(|e| format!("mcp flush: {e}"))?;
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let payload = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let line = format!("{payload}\n");
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("mcp write: {e}"))?;
            stdin.flush().await.map_err(|e| format!("mcp flush: {e}"))?;
        }
        tokio::time::timeout(REQUEST_TIMEOUT, self.read_response(id))
            .await
            .map_err(|_| format!("mcp `{method}` timed out"))?
    }

    /// Read lines until the response with the matching id arrives, skipping
    /// notifications and unrelated messages.
    async fn read_response(&self, id: i64) -> Result<Value, String> {
        let mut reader = self.reader.lock().await;
        loop {
            let mut line = String::new();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| format!("mcp read: {e}"))?;
            if n == 0 {
                return Err("mcp server closed the connection".into());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            if msg.get("id").and_then(|v| v.as_i64()) != Some(id) {
                continue;
            }
            return extract_result(&msg);
        }
    }
}

impl Drop for StdioClient {
    fn drop(&mut self) {
        // Best-effort kill so a server process does not outlive the session.
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP transport (Streamable HTTP / single JSON response)
// ---------------------------------------------------------------------------

pub struct HttpClient {
    url: String,
    headers: BTreeMap<String, String>,
    http: reqwest::Client,
    next_id: AtomicI64,
    session_id: Mutex<Option<String>>,
}

impl HttpClient {
    async fn connect(url: &str, headers: &BTreeMap<String, String>) -> Result<Self, String> {
        let client = HttpClient {
            url: url.to_string(),
            headers: headers.clone(),
            http: reqwest::Client::new(),
            next_id: AtomicI64::new(1),
            session_id: Mutex::new(None),
        };
        client.request("initialize", initialize_params()).await?;
        Ok(client)
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let payload = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let mut req = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        if let Some(sid) = self.session_id.lock().await.clone() {
            req = req.header("mcp-session-id", sid);
        }
        let resp = tokio::time::timeout(REQUEST_TIMEOUT, req.json(&payload).send())
            .await
            .map_err(|_| format!("mcp `{method}` timed out"))?
            .map_err(|e| format!("mcp http `{method}`: {e}"))?;
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            if let Ok(sid) = sid.to_str() {
                *self.session_id.lock().await = Some(sid.to_string());
            }
        }
        if !resp.status().is_success() {
            return Err(format!("mcp http `{method}`: status {}", resp.status()));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| format!("mcp http body: {e}"))?;
        let msg = parse_http_body(&body)?;
        extract_result(&msg)
    }
}

/// Accept either a bare JSON-RPC object or an SSE stream; in the SSE case take
/// the first `data:` line that parses as a JSON-RPC response.
fn parse_http_body(body: &str) -> Result<Value, String> {
    let trimmed = body.trim_start();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).map_err(|e| format!("mcp parse json: {e}"));
    }
    for line in body.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if v.get("jsonrpc").is_some() {
                    return Ok(v);
                }
            }
        }
    }
    Err("mcp http: no JSON-RPC payload in response".into())
}

/// Pull `result` out of a JSON-RPC envelope, surfacing `error` as `Err`.
fn extract_result(msg: &Value) -> Result<Value, String> {
    if let Some(err) = msg.get("error") {
        let message = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(format!("mcp error: {message}"));
    }
    msg.get("result")
        .cloned()
        .ok_or_else(|| "mcp response missing result".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_list_with_namespacing() {
        let result = json!({
            "tools": [
                { "name": "read_file", "description": "Read", "inputSchema": { "type": "object" } },
                { "name": "write_file" }
            ]
        });
        let specs = parse_tool_list("fs", &result).unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].qualified_name, "mcp.fs.read_file");
        assert_eq!(specs[0].remote_name, "read_file");
        assert_eq!(specs[1].qualified_name, "mcp.fs.write_file");
    }

    #[test]
    fn stringifies_text_content_blocks() {
        let result = json!({
            "content": [
                { "type": "text", "text": "hello" },
                { "type": "text", "text": "world" }
            ]
        });
        assert_eq!(stringify_tool_result(&result), "hello\nworld");
    }

    #[test]
    fn extract_result_surfaces_errors() {
        let msg = json!({ "jsonrpc": "2.0", "id": 1, "error": { "message": "boom" } });
        assert!(extract_result(&msg).unwrap_err().contains("boom"));
    }

    #[test]
    fn parses_sse_framed_http_body() {
        let body =
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n";
        let msg = parse_http_body(body).unwrap();
        assert_eq!(msg["result"]["ok"], json!(true));
    }
}
