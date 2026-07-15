use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("upstream timeout")]
    UpstreamTimeout,
    #[error("upstream request error: {0}")]
    UpstreamReq(#[from] reqwest::Error),
    #[error("upstream error: {status} {body}")]
    Upstream { status: u16, body: String },
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ApiError::UpstreamTimeout => (StatusCode::GATEWAY_TIMEOUT, "upstream timeout".to_string()),
            ApiError::UpstreamReq(e) => (StatusCode::BAD_GATEWAY, format!("upstream request: {e}")),
            ApiError::Upstream { status, body } => {
                let code = StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY);
                return (code, Json(json!({ "error": "upstream error", "status": status, "body": body }))).into_response();
            }
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self { ApiError::Internal(e.to_string()) }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self { ApiError::Internal(format!("io: {e}")) }
}
