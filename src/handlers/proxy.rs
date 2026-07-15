use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use crate::error::ApiError;
use crate::state::AppState;

static IMAGE_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9._\-/:@]+$").unwrap());

fn validate_image(image: &str) -> Result<(), ApiError> {
    if image.is_empty() {
        return Err(ApiError::BadRequest("missing image".to_string()));
    }
    if !IMAGE_NAME_RE.is_match(image) {
        return Err(ApiError::BadRequest("invalid image name".to_string()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ImageInfoRequest {
    pub image: String,
}

pub async fn image_info(
    State(state): State<AppState>,
    Json(req): Json<ImageInfoRequest>,
) -> Response {
    match proxy_get(&state, "info", &req.image, &[]).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ImageDownloadRequest {
    pub image: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub compressed: Option<bool>,
    #[serde(default)]
    pub platform: Option<String>,
}

pub async fn image_download(
    State(state): State<AppState>,
    Json(req): Json<ImageDownloadRequest>,
) -> Response {
    let pairs = [
        ("mode", req.mode.unwrap_or_else(|| "prepare".to_string())),
        (
            "compressed",
            req.compressed
                .unwrap_or(true)
                .to_string(),
        ),
        (
            "platform",
            req.platform.unwrap_or_else(|| "linux/amd64".to_string()),
        ),
    ];
    let pairs: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (*k, v.clone())).collect();
    match proxy_get(&state, "download", &req.image, &pairs).await {
        Ok(r) => r,
        Err(e) => e.into_response(),
    }
}

async fn proxy_get(
    state: &AppState,
    endpoint: &str,
    image: &str,
    extra: &[(&str, String)],
) -> Result<Response, ApiError> {
    if let Err(e) = validate_image(image) {
        return Err(e);
    }
    let mut url = format!("{}/api/image/{}?image={}", state.config.proxy_base_url, endpoint, urlencode(image));
    for (k, v) in extra {
        url.push_str(&format!("&{}={}", k, urlencode(v)));
    }

    let resp = match state.http.get(&url).send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return Err(ApiError::UpstreamTimeout),
        Err(e) => return Err(ApiError::Internal(format!("upstream request failed: {e}"))),
    };

    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or("application/json").to_string())
        .unwrap_or_else(|| "application/json".to_string());
    let body = resp
        .bytes()
        .await
        .map_err(|e| ApiError::Internal(format!("read upstream body: {e}")))?;

    if !status.is_success() {
        return Err(ApiError::Upstream {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&body).to_string(),
        });
    }

    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        reqwest::header::CONTENT_TYPE,
        content_type.parse().unwrap(),
    );
    Ok(response)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

#[allow(dead_code)]
fn _unused_timeout() -> Duration { Duration::from_secs(15) }
