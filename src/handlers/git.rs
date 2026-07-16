//! `POST /api/git/commit` — stage the on-disk `Dockerfile`, commit it, and
//! `git push` so the next GitHub Actions run picks up the latest edits.
//!
//! Designed to be called automatically before `/api/build` from the frontend
//! so the workflow always sees the freshest Dockerfile. We use `std::process`
//! to shell out to the system `git` binary rather than pulling in a Rust
//! git library.

use std::path::Path;
use std::process::Command;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use crate::error::ApiError;
use crate::state::AppState;

const DOCKERFILE_PATH: &str = "Dockerfile";

#[derive(Debug, Deserialize, Default)]
pub struct CommitRequest {
    /// Optional override of the commit message. Defaults to a short auto-generated
    /// string that names the upcoming build (if the caller passed those in).
    pub message: Option<String>,
    /// Allow callers to override the branch (defaults to config.github.default_branch).
    pub branch: Option<String>,
    /// If true, push to origin even if there are no local changes to commit.
    /// Default false: an empty commit is considered an error / no-op.
    #[serde(default)]
    pub allow_empty: bool,
}

pub async fn commit_dockerfile(
    State(state): State<AppState>,
    Json(req): Json<CommitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !Path::new(DOCKERFILE_PATH).exists() {
        return Err(ApiError::BadRequest(
            "Dockerfile not found on disk; save it first".into(),
        ));
    }
    let branch = req
        .branch
        .unwrap_or_else(|| state.config.github.default_branch.clone());
    let message = req.message.unwrap_or_else(|| {
        format!("autoimage: update Dockerfile ({} {DOCKERFILE_PATH})", chrono::Utc::now().to_rfc3339())
    });

    // 1. git add Dockerfile (only the file, not the whole tree)
    run_git(&["add", "--", DOCKERFILE_PATH])?;
    // 2. check whether the index actually has staged changes
    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet", "--", DOCKERFILE_PATH])
        .output()
        .map_err(|e| ApiError::Internal(format!("git diff failed to start: {e}")))?;
    if !diff.status.success() && diff.status.code() != Some(0) {
        // exit code 1 from `git diff --quiet` means there ARE differences;
        // 0 means clean. Anything else is an error.
        if diff.status.code() == Some(1) {
            // have staged changes — proceed
        } else {
            return Err(ApiError::Internal(format!(
                "git diff --cached failed: {}",
                String::from_utf8_lossy(&diff.stderr)
            )));
        }
    } else if !req.allow_empty {
        info!("commit_dockerfile: no staged changes, skipping commit+push");
        return Ok(Json(json!({
            "committed": false,
            "pushed": false,
            "message": "no changes to commit",
        })));
    }

    // 3. commit
    let commit_output = Command::new("git")
        .args(["commit", "-m", &message, "--", DOCKERFILE_PATH])
        .output()
        .map_err(|e| ApiError::Internal(format!("git commit failed to start: {e}")))?;
    if !commit_output.status.success() {
        return Err(ApiError::Internal(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit_output.stderr)
        )));
    }

    // 4. capture the new commit SHA
    let sha_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| ApiError::Internal(format!("git rev-parse failed: {e}")))?;
    if !sha_output.status.success() {
        return Err(ApiError::Internal(format!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&sha_output.stderr)
        )));
    }
    let sha = String::from_utf8_lossy(&sha_output.stdout).trim().to_string();

    // 5. push
    let push_output = Command::new("git")
        .args(["push", "origin", &branch])
        .output()
        .map_err(|e| ApiError::Internal(format!("git push failed to start: {e}")))?;
    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr).to_string();
        warn!("git push failed: {stderr}");
        return Err(ApiError::Upstream {
            status: 502,
            body: format!("git push origin {branch} failed: {stderr}"),
        });
    }

    info!(sha = %sha, branch = %branch, "commit_dockerfile: pushed {DOCKERFILE_PATH}");
    Ok(Json(json!({
        "committed": true,
        "pushed": true,
        "sha": sha,
        "branch": branch,
        "message": message,
    })))
}

fn run_git(args: &[&str]) -> Result<(), ApiError> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| ApiError::Internal(format!("git {args:?} failed to start: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::Internal(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
