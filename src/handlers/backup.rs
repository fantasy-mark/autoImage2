use std::path::PathBuf;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::backup as backup_mod;
use crate::error::ApiError;
use crate::state::AppState;

fn resolve_dir(path: &std::path::Path) -> PathBuf {
    match path.parent() {
        Some(p) if p.as_os_str().is_empty() => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

#[derive(Debug, Serialize)]
pub struct BackupListResponse {
    pub backups: Vec<backup_mod::BackupEntry>,
}

pub async fn list_backups(State(state): State<AppState>) -> Result<Json<BackupListResponse>, ApiError> {
    let dir = resolve_dir(&state.dockerfile_path);
    let backups = backup_mod::list_backups(&dir)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(BackupListResponse { backups }))
}

#[derive(Debug, Serialize)]
pub struct BackupContentResponse {
    pub name: String,
    pub content: String,
}

pub async fn get_backup(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<BackupContentResponse>, ApiError> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ApiError::BadRequest("invalid backup name".to_string()));
    }
    if !backup_mod::is_backup_name(&name) {
        return Err(ApiError::BadRequest("invalid backup name".to_string()));
    }
    let dir = resolve_dir(&state.dockerfile_path);
    let path = dir.join(&name);
    if !path.exists() {
        return Err(ApiError::NotFound);
    }
    let bytes = std::fs::read(&path).map_err(|e| ApiError::Internal(e.to_string()))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| ApiError::Internal("backup is not valid UTF-8".to_string()))?;
    Ok(Json(BackupContentResponse { name, content }))
}
