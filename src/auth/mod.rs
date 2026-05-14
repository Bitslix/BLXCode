//! Better Auth über HTTP (`/api/auth/...`): Session-Check, Login, Device Flow, Sign-out.
mod login_modal;

pub use login_modal::LoginModal;

use crate::config::{API_URL, AUTH_DEVICE_CLIENT_ID};
use crate::service::{ApiError, ApiService};
use gloo_net::http::Response;
use leptos::prelude::{GetUntracked, RwSignal, Set};
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// OAuth 2.0 Device Authorization Grant (RFC 8628).
pub const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// Relativer API-Pfad, z. B. `auth/get-session`.
#[must_use]
pub fn auth_path(segment: &str) -> String {
    format!(
        "{}/{}",
        crate::config::AUTH_PATH_PREFIX.trim_matches('/'),
        segment.trim_start_matches('/'),
    )
}

/// Fehler beim Auth‑Flow (Transport, Parsing, geschäftliche Meldungen).
#[derive(Debug, Error)]
pub enum AuthError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

impl From<gloo_net::Error> for AuthError {
    fn from(e: gloo_net::Error) -> Self {
        AuthError::Api(ApiError::from(e))
    }
}

async fn response_text(resp: Response) -> Result<String, gloo_net::Error> {
    resp.text().await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BetterAuthRpcErrorBody {
    message: Option<String>,
}

impl BetterAuthRpcErrorBody {
    fn into_message_fallback(self, status: u16) -> String {
        self.message
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("HTTP {status}"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionUserPayload {
    id: String,
    name: Option<String>,
    email: Option<String>,
    image: Option<String>,
}

/// Anzeigedaten aus Better-Auth-`get-session` (`user`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthUserBrief {
    pub name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

/// Liest `/get-session`; liefert `None`, wenn keine Sitzung.
pub async fn fetch_auth_session(
    api: &ApiService,
    bearer: Option<&str>,
) -> Result<Option<AuthUserBrief>, AuthError> {
    let resp = api
        .get_credentialed(&auth_path("get-session"), bearer)
        .await?;
    let status = resp.status();
    let text = response_text(resp).await.map_err(AuthError::from)?;
    if !(200..300).contains(&status) {
        return Err(AuthError::Message(format!(
            "get-session HTTP {status}: {}",
            text.trim()
        )));
    }
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(None);
    }
    let v: serde_json::Value = serde_json::from_str(trimmed)?;
    let user = match v.get("user") {
        Some(u) if !u.is_null() => u,
        _ => return Ok(None),
    };
    let u: SessionUserPayload = serde_json::from_value(user.clone())?;
    let name = u
        .name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| u.email.clone().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| u.id.clone());
    let email = u.email.filter(|s| !s.trim().is_empty());
    let avatar_url = u.image.filter(|s| !s.trim().is_empty());
    Ok(Some(AuthUserBrief {
        name,
        email,
        avatar_url,
    }))
}

/// `true`, wenn `/get-session` ein `user`-Objekt liefert (nicht `null`).
#[allow(dead_code)]
pub async fn is_logged_in(api: &ApiService, bearer: Option<&str>) -> Result<bool, AuthError> {
    Ok(fetch_auth_session(api, bearer).await?.is_some())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInEmailPayload {
    pub email: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remember_me: Option<bool>,
}

pub async fn sign_in_email(
    api: &ApiService,
    payload: &SignInEmailPayload,
    bearer: Option<&str>,
) -> Result<(), AuthError> {
    let resp = api
        .post_json_credentialed(&auth_path("sign-in/email"), payload, bearer)
        .await?;
    let status = resp.status();
    let text = response_text(resp).await.map_err(AuthError::from)?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let msg = serde_json::from_str::<BetterAuthRpcErrorBody>(&text)
        .map(|e| e.into_message_fallback(status))
        .unwrap_or_else(|_| format!("HTTP {status}"));
    Err(AuthError::Message(msg))
}

pub async fn sign_out(api: &ApiService, bearer: Option<&str>) -> Result<(), AuthError> {
    let resp = api
        .post_credentialed_empty(&auth_path("sign-out"), bearer)
        .await?;
    let status = resp.status();
    if !(200..300).contains(&status) {
        let text = response_text(resp).await.unwrap_or_default();
        return Err(AuthError::Message(format!(
            "sign-out HTTP {status}: {}",
            text.trim()
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DeviceCodeRequest<'a> {
    pub client_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub interval: u64,
}

pub async fn request_device_code(
    api: &ApiService,
    scope: Option<&str>,
    bearer: Option<&str>,
) -> Result<DeviceCodeResponse, AuthError> {
    let body = DeviceCodeRequest {
        client_id: AUTH_DEVICE_CLIENT_ID,
        scope,
    };
    let resp = api
        .post_json_credentialed(&auth_path("device/code"), &body, bearer)
        .await?;
    let status = resp.status();
    let text = response_text(resp).await.map_err(AuthError::from)?;
    if !(200..300).contains(&status) {
        let msg = serde_json::from_str::<DeviceTokenErrBody>(&text)
            .ok()
            .map(|e| e.error_description.unwrap_or(e.error))
            .unwrap_or_else(|| format!("HTTP {status}: {}", text.trim()));
        return Err(AuthError::Message(msg));
    }
    Ok(serde_json::from_str(&text)?)
}

#[derive(Debug, Serialize)]
pub struct DeviceTokenRequest<'a> {
    pub grant_type: &'static str,
    pub device_code: &'a str,
    pub client_id: &'a str,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeviceTokenSuccess {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceTokenErrBody {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

pub enum DevicePollOutcome {
    Success(DeviceTokenSuccess),
    AuthorizationPending,
    SlowDown,
    Denied(String),
}

/// Ein einzelner Poll gegen `/device/token`.
pub async fn poll_device_token(
    api: &ApiService,
    device_code: &str,
    bearer: Option<&str>,
) -> Result<DevicePollOutcome, AuthError> {
    let body = DeviceTokenRequest {
        grant_type: DEVICE_GRANT_TYPE,
        device_code,
        client_id: AUTH_DEVICE_CLIENT_ID,
    };
    let resp = api
        .post_json_credentialed(&auth_path("device/token"), &body, bearer)
        .await?;
    let status = resp.status();
    let text = response_text(resp).await.map_err(AuthError::from)?;
    if (200..300).contains(&status) {
        let ok: DeviceTokenSuccess = serde_json::from_str(&text)?;
        return Ok(DevicePollOutcome::Success(ok));
    }
    let err: DeviceTokenErrBody = serde_json::from_str(&text)
        .map_err(|_| AuthError::Message(format!("device/token HTTP {status}: {}", text.trim())))?;
    match err.error.as_str() {
        "authorization_pending" => Ok(DevicePollOutcome::AuthorizationPending),
        "slow_down" => Ok(DevicePollOutcome::SlowDown),
        "access_denied" => Ok(DevicePollOutcome::Denied(
            err.error_description
                .unwrap_or_else(|| "Zugriff verweigert".into()),
        )),
        "expired_token" => Ok(DevicePollOutcome::Denied(
            err.error_description
                .unwrap_or_else(|| "Code abgelaufen".into()),
        )),
        other => Err(AuthError::Message(
            err.error_description.unwrap_or_else(|| other.to_string()),
        )),
    }
}

/// Macht eine relative Verifikations-URL absolut (Basis: [`API_URL`]).
#[must_use]
pub fn verification_url_open(verification_uri: &str) -> String {
    let t = verification_uri.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return t.to_string();
    }
    let base = API_URL.trim_end_matches('/');
    if t.starts_with('/') {
        format!("{base}{t}")
    } else {
        format!("{base}/{t}")
    }
}

fn open_via_dom_window(url: &str) {
    let Some(win) = web_sys::window() else {
        return;
    };
    let opened = win.open_with_url_and_target(url, "_blank").ok().flatten();
    if opened.is_none() {
        let _ = win.location().set_href(url);
    }
}

/// Öffnet die Verifikations-URL im **Systembrowser**.
///
/// Nach `await` blockieren WebViews `window.open()` wegen Popup-Policy. Unter Tauri daher [`crate::tauri_bridge::open_external_url`].
pub fn open_in_new_tab(url: &str) {
    if crate::tauri_bridge::is_tauri_shell() {
        let owned = url.to_string();
        spawn_local(async move {
            if crate::tauri_bridge::open_external_url(&owned)
                .await
                .is_err()
            {
                open_via_dom_window(&owned);
            }
        });
        return;
    }
    open_via_dom_window(url);
}

#[derive(Clone, PartialEq)]
pub enum AuthGateState {
    CheckingSession,
    NeedLogin,
    LoggedIn,
}

/// Leptos-Kontext: Gate, Profil, optionaler Bearer und Abmelde-Callback.
#[derive(Copy, Clone)]
pub struct AuthEnv {
    pub gate: RwSignal<AuthGateState>,
    pub bearer: RwSignal<Option<String>>,
    pub profile: RwSignal<Option<AuthUserBrief>>,
    pub logout: leptos::callback::UnsyncCallback<(), ()>,
}

/// Aktualisiert [`AuthGateState::LoggedIn`] und [`AuthEnv::profile`], wenn `/get-session` einen Nutzer sieht.
pub async fn promote_if_session_valid(auth: AuthEnv, api: &ApiService) -> Result<bool, AuthError> {
    let bt = auth.bearer.get_untracked();
    match fetch_auth_session(api, bt.as_deref()).await? {
        Some(p) => {
            auth.profile.set(Some(p));
            auth.gate.set(AuthGateState::LoggedIn);
            Ok(true)
        }
        None => Ok(false),
    }
}
