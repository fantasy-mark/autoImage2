use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::backup;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DockerfileResponse {
    pub content: String,
    pub size: u64,
    pub updated_at: String,
}

pub async fn get_dockerfile(State(state): State<AppState>) -> Result<Json<DockerfileResponse>, ApiError> {
    let path = state.dockerfile_path.as_ref();
    if !path.exists() {
        return Err(ApiError::NotFound);
    }
    let bytes = std::fs::read(path).map_err(|e| ApiError::Internal(e.to_string()))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| ApiError::Internal("file is not valid UTF-8".to_string()))?;
    let meta = std::fs::metadata(path).map_err(|e| ApiError::Internal(e.to_string()))?;
    let updated_at = meta
        .modified()
        .ok()
        .map(|t| DateTime::<Local>::from(t).to_rfc3339())
        .unwrap_or_default();
    Ok(Json(DockerfileResponse {
        content,
        size: meta.len(),
        updated_at,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PutDockerfileRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PutDockerfileResponse {
    pub size: u64,
    pub backup: String,
}

pub async fn put_dockerfile(
    State(state): State<AppState>,
    Json(req): Json<PutDockerfileRequest>,
) -> Result<Json<PutDockerfileResponse>, ApiError> {
    // UTF-8 validation: rejecting non-utf8 here returns 400 per the spec.
    let _ = String::from_utf8(req.content.as_bytes().to_vec())
        .map_err(|_| ApiError::BadRequest("content must be utf-8".to_string()))?;

    let path = state.dockerfile_path.clone();
    let dir = match path.parent() {
        Some(p) if p.as_os_str().is_empty() => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    };

    let backup_path = backup::backup_dockerfile(&dir)
        .map_err(|e| ApiError::Internal(format!("backup failed: {e}")))?;
    let backup_name = backup_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let bytes = req.content.into_bytes();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    std::fs::write(path.as_ref(), &bytes)
        .map_err(|e| ApiError::Internal(format!("write failed: {e}")))?;

    Ok(Json(PutDockerfileResponse {
        size: bytes.len() as u64,
        backup: backup_name,
    }))
}
