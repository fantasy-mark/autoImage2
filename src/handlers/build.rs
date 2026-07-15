use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    pub image: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BuildResponse {
    pub accepted: bool,
    pub workflow: String,
}

pub async fn trigger_build(
    State(state): State<AppState>,
    Json(req): Json<BuildRequest>,
) -> Response {
    match dispatch(&state, req).await {
        Ok(r) => (StatusCode::ACCEPTED, Json(r)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn dispatch(state: &AppState, req: BuildRequest) -> Result<BuildResponse, ApiError> {
    let token = std::env::var("GH_TOKEN")
        .map_err(|_| ApiError::Internal("GH_TOKEN not configured".to_string()))?;
    if token.is_empty() {
        return Err(ApiError::Internal("GH_TOKEN not configured".to_string()));
    }
    let cfg = &state.config.github;
    if cfg.owner.is_empty() || cfg.repo.is_empty() {
        return Err(ApiError::Internal(
            "github.owner / github.repo not configured".to_string(),
        ));
    }

    let image = req.image.unwrap_or_default();
    let version = req.version.unwrap_or_default();

    let url = format!(
        "https://api.github.com/repos/{}/{}/actions/workflows/{}/dispatches",
        cfg.owner, cfg.repo, cfg.workflow_file
    );

    let body = json!({
        "ref": cfg.default_branch,
        "inputs": {
            "repo": state.config.target.repo,
            "namespace": state.config.namespace(),
            "image": image,
            "version": version,
        }
    });

    let resp = state
        .http
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", format!("autoimage/{}", env!("CARGO_PKG_VERSION")))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ApiError::UpstreamTimeout
            } else {
                ApiError::Internal(format!("github api request failed: {e}"))
            }
        })?;

    let status = resp.status();
    if status.is_success() {
        return Ok(BuildResponse {
            accepted: true,
            workflow: cfg.workflow_file.clone(),
        });
    }
    let text = resp.text().await.unwrap_or_default();
    tracing::warn!(%status, body = %text, "github dispatch failed");
    Err(ApiError::Upstream {
        status: status.as_u16(),
        body: text,
    })
}
