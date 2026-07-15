//! Download a built image from ghcr.io as a `docker load`-able tar.
//!
//! Flow:
//! 1. `GET /v2/{ns}/{name}/manifests/{tag}` — negotiate content type, parse the manifest
//! 2. `GET /v2/{ns}/{name}/blobs/{config_digest}` — pull the image config JSON
//! 3. For each layer in order: `GET /v2/{ns}/{name}/blobs/{layer_digest}` — stream the layer
//! 4. Assemble a Docker `docker save` tar in memory and stream it as the response body
//!
//! Authentication: the upstream `ghcr.io` lets public images through without
//! `Authorization`. For private packages we forward the app's `GH_TOKEN` as a
//! Bearer (requires `read:packages` on the token).

use std::collections::BTreeMap;
use std::io::Write as IoWrite;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, warn};

use crate::error::ApiError;
use crate::state::AppState;

const REGISTRY_HOST: &str = "ghcr.io";
const DEFAULT_NAMESPACE: &str = "fantasy-mark";
// We accept the manifests in both OCI and Docker v2 forms, plus the index/list
// forms so a single-arch `docker pull`-style request can be resolved.
const MANIFEST_ACCEPT: &str = "application/vnd.docker.distribution.manifest.v2+json,\
application/vnd.docker.distribution.manifest.list.v2+json,\
application/vnd.oci.image.manifest.v1+json,\
application/vnd.oci.image.index.v1+json";
const DOCKER_MANIFEST_V2: &str = "application/vnd.docker.distribution.manifest.v2+json";
const DOCKER_MANIFEST_LIST: &str = "application/vnd.docker.distribution.manifest.list.v2+json";
const OCI_MANIFEST_V1: &str = "application/vnd.oci.image.manifest.v1+json";
const OCI_INDEX_V1: &str = "application/vnd.oci.image.index.v1+json";

#[derive(Debug, Deserialize)]
pub struct DownloadParams {
    pub image: String,
    pub version: Option<String>,
    /// optional registry namespace, defaults to github.owner
    pub namespace: Option<String>,
    /// optional override of the registry host (for tests / future-proofing)
    pub registry: Option<String>,
}

pub async fn download_image(
    State(state): State<AppState>,
    Query(params): Query<DownloadParams>,
) -> Response {
    let image = params.image.trim().to_string();
    if image.is_empty() {
        return ApiError::BadRequest("image must be non-empty".into()).into_response();
    }
    let tag = params
        .version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("latest")
        .to_string();
    let namespace = params
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&state.config.github.owner)
        .to_string();
    let registry = params.registry.unwrap_or_else(|| REGISTRY_HOST.to_string());

    let token = std::env::var("GH_TOKEN").ok().filter(|s| !s.is_empty());

    match build_tar(&state, &registry, &namespace, &image, &tag, token).await {
        Ok(bytes) => {
            let filename = format!("{}_{}.tar", image, sanitize_tag(&tag));
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/x-tar"),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!(
                    "attachment; filename=\"{}\"",
                    filename
                ))
                .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
            );
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from(bytes.len() as u64));
            (StatusCode::OK, headers, Body::from(bytes)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

fn sanitize_tag(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

#[derive(serde::Deserialize, Debug)]
struct Manifest {
    #[serde(default)]
    config: Option<Descriptor>,
    /// for manifest lists / OCI indexes: list of per-platform manifests
    #[serde(default)]
    manifests: Vec<Descriptor>,
    /// for v1 OCI: layers
    #[serde(default)]
    layers: Vec<Descriptor>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct Descriptor {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    platform: Option<Platform>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct Platform {
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    os: Option<String>,
}

/// One-shot registry credentials we obtain via the Docker Registry v2 bearer
/// challenge dance. We forward the GitHub PAT (if any) so the exchange endpoint
/// can mint a token with `read:packages`-equivalent scope when applicable.
#[derive(Clone, Default)]
struct RegistryCreds {
    /// Token to send as `Authorization: Bearer <token>` on registry calls.
    bearer: Option<String>,
}

/// Parse a WWW-Authenticate Bearer challenge.
fn parse_bearer_challenge(value: &str) -> Option<(String, Option<String>, Vec<(String, String)>)> {
    // "Bearer realm=\"https://...\",service=\"...\",scope=\"...\""
    let rest = value.trim().strip_prefix("Bearer").or_else(|| value.trim().strip_prefix("bearer"))?;
    let mut realm: Option<String> = None;
    let mut service: Option<String> = None;
    let mut params: Vec<(String, String)> = Vec::new();
    for part in rest.split(',') {
        let part = part.trim();
        let (k, v) = match part.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        let key = k.trim().to_ascii_lowercase();
        let raw = v.trim();
        let val = raw.trim_matches('"').to_string();
        match key.as_str() {
            "realm" => realm = Some(val.clone()),
            "service" => service = Some(val.clone()),
            _ => params.push((key, val.clone())),
        }
    }
    realm.map(|r| (r, service, params))
}

/// Perform the bearer-token exchange against the realm URL. The `github_pat`
/// (if provided) is forwarded as `Authorization: Bearer ...` so that ghcr.io
/// can scope the issued token to the GitHub user's grants. For public packages
/// no auth is required and the server still returns a valid pull token.
async fn exchange_token(
    http: &reqwest::Client,
    realm: &str,
    service: Option<&str>,
    scope: Option<&str>,
    github_pat: Option<&str>,
) -> Result<String, ApiError> {
    // realm is a full URL; only the query parameters need to be percent-encoded.
    let mut url = realm.trim_end_matches('?').to_string();
    let mut qp: Vec<String> = Vec::new();
    if let Some(s) = service { qp.push(format!("service={}", urlencoding::encode(s))); }
    if let Some(s) = scope { qp.push(format!("scope={}", urlencoding::encode(s))); }
    if !qp.is_empty() {
        url.push('?');
        url.push_str(&qp.join("&"));
    }
    let mut req = http.get(&url).header("User-Agent", concat!("autoimage/", env!("CARGO_PKG_VERSION")));
    if let Some(t) = github_pat {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let resp = req.send().await.map_err(|e| ApiError::UpstreamReq(e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Upstream { status: status.as_u16(), body });
    }
    #[derive(serde::Deserialize)]
    struct TokenResp { token: String }
    let tr: TokenResp = resp.json().await.map_err(|e| ApiError::UpstreamReq(e))?;
    Ok(tr.token)
}

/// Send a GET against the registry. On 401, perform the bearer-token exchange
/// using the challenge from `WWW-Authenticate`, then retry once.
async fn registry_get(
    http: &reqwest::Client,
    url: &str,
    accept: &str,
    creds: &mut RegistryCreds,
    github_pat: Option<&str>,
) -> Result<reqwest::Response, ApiError> {
    let mut attempt = 0u8;
    loop {
        let mut req = http
            .get(url)
            .header(header::ACCEPT, accept)
            .header("User-Agent", concat!("autoimage/", env!("CARGO_PKG_VERSION")));
        if let Some(t) = creds.bearer.as_deref() {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let resp = req.send().await.map_err(|e| ApiError::UpstreamReq(e))?;
        if resp.status().as_u16() == 401 && attempt == 0 {
            // parse the challenge
            let challenge = resp
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            // consume the body so the connection is reusable
            let _ = resp.bytes().await;
            if let Some(challenge) = challenge {
                if let Some((realm, service, params)) = parse_bearer_challenge(&challenge) {
                    let scope = params.iter().find(|(k, _)| k == "scope").map(|(_, v)| v.as_str());
                    match exchange_token(http, &realm, service.as_deref(), scope, github_pat).await {
                        Ok(t) => {
                            creds.bearer = Some(t);
                            attempt += 1;
                            continue;
                        }
                        Err(e) => {
                            warn!("registry token exchange failed: {e}");
                            return Err(e);
                        }
                    }
                }
            }
            return Err(ApiError::Upstream { status: 401, body: "no bearer challenge".into() });
        }
        return Ok(resp);
    }
}

/// Pull manifest (follows index/list if needed) and return the list of layer
/// digests plus the config digest.
async fn fetch_manifest(
    http: &reqwest::Client,
    registry: &str,
    ns: &str,
    repo: &str,
    tag: &str,
    github_pat: Option<&str>,
) -> Result<(String, Vec<Descriptor>, Descriptor), ApiError> {
    let mut creds = RegistryCreds::default();
    let url = format!("https://{registry}/v2/{ns}/{repo}/manifests/{tag}");
    let resp = registry_get(http, &url, MANIFEST_ACCEPT, &mut creds, github_pat).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Upstream { status: status.as_u16(), body });
    }
    let ctype = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let manifest: Manifest = resp
        .json()
        .await
        .map_err(|e| ApiError::UpstreamReq(e))?;
    debug!("manifest media_type={:?} ctype={} config={:?} layers={} manifests={}",
        manifest.layers.len(), ctype, manifest.config.is_some(), manifest.manifests.len(), 0);

    if ctype == DOCKER_MANIFEST_V2 || ctype == OCI_MANIFEST_V1 {
        // single-arch: we need a config descriptor and a layer list
        let config = manifest
            .config
            .ok_or_else(|| ApiError::Upstream { status: 502, body: "manifest missing config".into() })?;
        Ok((ctype, manifest.layers, config))
    } else if ctype == DOCKER_MANIFEST_LIST || ctype == OCI_INDEX_V1 {
        // multi-arch: pick the first linux/amd64 manifest, recurse
        let chosen = manifest
            .manifests
            .iter()
            .find(|m| {
                m.platform
                    .as_ref()
                    .map(|p| {
                        (p.os.as_deref() == Some("linux"))
                            && (p.architecture.as_deref() == Some("amd64")
                                || p.architecture.as_deref() == Some("x86_64"))
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .or_else(|| manifest.manifests.first().cloned())
            .ok_or_else(|| ApiError::Upstream { status: 502, body: "no manifests in index".into() })?;
        // Fetch the inner manifest by digest. ghcr.io accepts the digest in the URL path
        // AND still requires the right Accept header. Reuse the creds we already obtained.
        let inner_url = format!(
            "https://{registry}/v2/{ns}/{repo}/manifests/{}",
            chosen.digest
        );
        let inner_resp = registry_get(http, &inner_url, MANIFEST_ACCEPT, &mut creds, github_pat).await?;
        let inner_status = inner_resp.status();
        tracing::info!(status = inner_status.as_u16(), ctype = ?inner_resp.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()), "inner manifest response");
        if !inner_status.is_success() {
            let body = inner_resp.text().await.unwrap_or_default();
            return Err(ApiError::Upstream { status: inner_status.as_u16(), body });
        }
        let inner_manifest: Manifest = inner_resp
            .json()
            .await
            .map_err(|e| ApiError::UpstreamReq(e))?;
        let config = inner_manifest.config.ok_or_else(|| ApiError::Upstream {
            status: 502,
            body: "inner manifest missing config".into(),
        })?;
        Ok((DOCKER_MANIFEST_V2.to_string(), inner_manifest.layers, config))
    } else {
        Err(ApiError::Upstream { status: 502, body: format!("unexpected manifest content-type: {ctype}") })
    }
}

async fn fetch_blob(
    http: &reqwest::Client,
    registry: &str,
    ns: &str,
    repo: &str,
    digest: &str,
    creds: &mut RegistryCreds,
    github_pat: Option<&str>,
) -> Result<reqwest::Response, ApiError> {
    let url = format!("https://{registry}/v2/{ns}/{repo}/blobs/{digest}");
    registry_get(http, &url, "*/*", creds, github_pat).await
}

async fn build_tar(
    state: &AppState,
    registry: &str,
    ns: &str,
    image: &str,
    tag: &str,
    token: Option<String>,
) -> Result<Vec<u8>, ApiError> {
    let mut creds = RegistryCreds::default();
    // 1) manifest
    let (_ctype, layers, config) =
        fetch_manifest(&state.http, registry, ns, image, tag, token.as_deref()).await?;
    debug!(
        "image={}:{} → {} layers, config {}",
        image,
        tag,
        layers.len(),
        config.digest
    );

    // 2) config
    let config_resp =
        fetch_blob(&state.http, registry, ns, image, &config.digest, &mut creds, token.as_deref()).await?;
    let config_bytes = config_resp
        .bytes()
        .await
        .map_err(|e| ApiError::UpstreamReq(e))?
        .to_vec();
    // Derive a stable image-id: sha256 of the config JSON, stripped of "sha256:" prefix
    let image_id = derive_id_from_config(&config_bytes).unwrap_or_else(|| {
        config
            .digest
            .trim_start_matches("sha256:")
            .to_string()
    });

    // 3) layers — pre-fetch all (small images only) so we can assemble one tar.
    // For large images this should be replaced with a streaming pipeline.
    let mut layer_bytes: Vec<(String, Vec<u8>)> = Vec::with_capacity(layers.len());
    for (i, layer) in layers.iter().enumerate() {
        let layer_name = if i == 0 {
            "layer.tar".to_string()
        } else {
            format!("layer.tar.{}", i)
        };
        debug!("layer {}/{}: {}", i + 1, layers.len(), layer.digest);
        let resp = fetch_blob(&state.http, registry, ns, image, &layer.digest, &mut creds, token.as_deref()).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ApiError::UpstreamReq(e))?
            .to_vec();
        layer_bytes.push((layer_name, bytes));
    }

    // 4) assemble tar
    let mut buf: Vec<u8> = Vec::with_capacity(
        1024 + config_bytes.len() + layer_bytes.iter().map(|(_, b)| b.len()).sum::<usize>() + 4096,
    );
    let manifest_json = serde_json::to_vec_pretty(&json!([{
        "Config": format!("{image_id}/config.json"),
        "RepoTags": [format!("{ns}/{image}:{tag}")],
        "Layers": layer_bytes.iter().map(|(name, _)| format!("{image_id}/{name}")).collect::<Vec<_>>(),
    }]))
    .map_err(|e| ApiError::Internal(format!("serialize manifest: {e}")))?;
    // repositories (v1-compat, optional but docker load is happy with it)
    let repositories_json = serde_json::to_vec(&json!({
        format!("{ns}/{image}"): {
            tag: image_id.clone()
        }
    }))
    .map_err(|e| ApiError::Internal(format!("serialize repositories: {e}")))?;

    append_tar_file(&mut buf, "manifest.json", &manifest_json)?;
    append_tar_file(&mut buf, "repositories", &repositories_json)?;
    append_tar_file(&mut buf, &format!("{image_id}/config.json"), &config_bytes)?;
    for (name, bytes) in &layer_bytes {
        append_tar_file(&mut buf, &format!("{image_id}/{name}"), bytes)?;
    }
    append_tar_end(&mut buf)?;
    Ok(buf)
}

fn derive_id_from_config(config: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(config).ok()?;
    // Docker v2 config puts it in `.id` (sha256 hex, no algorithm prefix)
    if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
        return Some(id.to_string());
    }
    // OCI image config: `digest` field is "sha256:hex"
    if let Some(d) = v.get("digest").and_then(|x| x.as_str()) {
        return Some(d.trim_start_matches("sha256:").to_string());
    }
    None
}

/// Build a ustar header for `name` and append it + the file payload + padding.
fn append_tar_file(buf: &mut Vec<u8>, name: &str, data: &[u8]) -> std::io::Result<()> {
    let mut header = [0u8; 512];
    // name (truncate to 100)
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(100);
    header[..name_len].copy_from_slice(&name_bytes[..name_len]);
    // mode: 0o644 = 0o100000 + 0o644 = 0o100644
    write_octal(&mut header[100..108], 0o100644);
    // uid / gid = 0
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    // size
    write_octal(&mut header[124..136], data.len() as u64);
    // mtime = 0
    write_octal(&mut header[136..148], 0);
    // Fill chksum with spaces, then write the rest of the header fields, THEN
    // compute the sum so the typeflag/magic/uname/gname bytes are included.
    for b in &mut header[148..156] { *b = b' '; }
    // typeflag = '0' (regular file)
    header[156] = b'0';
    // magic = "ustar\0" + version = "00"
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    // uname / gname = "root"
    let uname = b"root";
    header[265..265 + uname.len()].copy_from_slice(uname);
    let gname = b"root";
    header[297..297 + gname.len()].copy_from_slice(gname);

    let checksum: u32 = header.iter().map(|&b| b as u32).sum();
    write_octal(&mut header[148..156], checksum as u64);

    buf.extend_from_slice(&header);
    buf.extend_from_slice(data);
    // pad to 512 bytes
    let pad = (512 - (data.len() % 512)) % 512;
    if pad > 0 {
        let pad_block = vec![0u8; pad];
        buf.extend_from_slice(&pad_block);
    }
    Ok(())
}

fn write_octal(dst: &mut [u8], value: u64) {
    // ustar octal fields are NUL-terminated, the LAST byte of the field is '\0'
    // (or ' ' for older tar variants). We use the NUL-terminated form.
    let s = format!("{value:o}");
    let len = dst.len();
    if s.len() > len - 1 {
        // overflow — write as many 7s as we can
        for b in &mut dst[..len - 1] { *b = b'7'; }
        dst[len - 1] = 0;
        return;
    }
    for b in &mut dst[..len - 1 - s.len()] { *b = b'0'; }
    let pos = len - 1 - s.len();
    dst[pos..pos + s.len()].copy_from_slice(s.as_bytes());
    dst[len - 1] = 0;
}

fn append_tar_end(buf: &mut Vec<u8>) -> std::io::Result<()> {
    // Two 512-byte zero blocks mark end-of-archive
    buf.extend_from_slice(&[0u8; 1024]);
    Ok(())
}
