//! HTTP client for the backend API (browser `fetch` via `gloo-net`).
//!
//! Call sites will use [`ApiService`] via Leptos context.

#![allow(dead_code)]

use gloo_net::http::{Request, Response};
use serde::Serialize;
use thiserror::Error;
use web_sys::RequestCredentials;

fn with_credentials_and_bearer(
    mut builder: gloo_net::http::RequestBuilder,
    bearer: Option<&str>,
) -> gloo_net::http::RequestBuilder {
    builder = builder.credentials(RequestCredentials::Include);
    if let Some(t) = bearer {
        let t = t.trim();
        if !t.is_empty() {
            builder = builder.header("Authorization", &format!("Bearer {}", t));
        }
    }
    builder
}

/// Errors returned by [`ApiService`] HTTP calls.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Underlying network or serialization error from `gloo-net`.
    #[error(transparent)]
    Network(#[from] gloo_net::Error),
}

fn join_api_base(api_url: &str, api_path: &str) -> String {
    format!(
        "{}/{}",
        api_url.trim_end_matches('/'),
        api_path.trim_matches('/')
    )
}

/// HTTP client using [`crate::config::API_URL`] and [`crate::config::API_PATH`] as the request prefix.
#[derive(Debug, Clone)]
pub struct ApiService {
    base_url: String,
}

impl ApiService {
    /// Builds a client using [`crate::config::API_URL`] and [`crate::config::API_PATH`].
    pub fn new() -> Self {
        Self {
            base_url: join_api_base(crate::config::API_URL, crate::config::API_PATH),
        }
    }

    /// Full URL for a path segment relative to the configured API base (leading slashes ignored).
    pub fn endpoint(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    /// Sends a GET request to [`Self::endpoint`] for `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError`] if the request fails.
    pub async fn get(&self, path: &str) -> Result<Response, ApiError> {
        let url = self.endpoint(path);
        Ok(Request::get(&url).send().await?)
    }

    /// Sends a POST request with a JSON body to [`Self::endpoint`] for `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError`] if serialization or the request fails.
    pub async fn post_json<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<Response, ApiError> {
        let url = self.endpoint(path);
        let request = Request::post(&url).json(body)?;
        Ok(request.send().await?)
    }

    /// GET with cookies and HTTP auth headers (Better Auth sessions).
    pub async fn get_credentialed(
        &self,
        path: &str,
        bearer: Option<&str>,
    ) -> Result<Response, ApiError> {
        let url = self.endpoint(path);
        Ok(with_credentials_and_bearer(Request::get(&url), bearer)
            .send()
            .await?)
    }

    /// POST JSON with cookies and optional `Authorization: Bearer`.
    pub async fn post_json_credentialed<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
        bearer: Option<&str>,
    ) -> Result<Response, ApiError> {
        let url = self.endpoint(path);
        let req = with_credentials_and_bearer(Request::post(&url), bearer).json(body)?;
        Ok(req.send().await?)
    }

    /// POST without JSON body (empty body) but with credentials.
    pub async fn post_credentialed_empty(
        &self,
        path: &str,
        bearer: Option<&str>,
    ) -> Result<Response, ApiError> {
        let url = self.endpoint(path);
        Ok(with_credentials_and_bearer(Request::post(&url), bearer)
            .header("Content-Type", "application/json")
            .body("{}")?
            .send()
            .await?)
    }
}

impl Default for ApiService {
    fn default() -> Self {
        Self::new()
    }
}
