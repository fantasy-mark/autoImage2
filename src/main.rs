use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod backup;
mod config;
mod error;
mod handlers;
mod state;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::load().context("loading config.toml")?;
    info!(
        bind = %config.bind,
        proxy_base_url = %config.proxy_base_url,
        owner = %config.github.owner,
        repo = %config.github.repo,
        workflow = %config.github.workflow_file,
        target_repo = %config.target.repo,
        target_namespace = %config.namespace(),
        "autoimage starting"
    );

    if std::env::var("GH_TOKEN").is_err() {
        warn!("GH_TOKEN is not set; POST /api/build will return 500 until it is provided");
    }

    let state = AppState::new(config.clone());

    let static_dir = static_dir_path();
    info!(path = %static_dir.display(), "serving static files");

    let app = Router::new()
        .route("/", get(handlers::index::index))
        .route(
            "/api/dockerfile",
            get(handlers::dockerfile::get_dockerfile).put(handlers::dockerfile::put_dockerfile),
        )
        .route("/api/dockerfile/backups", get(handlers::backup::list_backups))
        .route(
            "/api/dockerfile/backups/:name",
            get(handlers::backup::get_backup),
        )
        .route("/api/image/info", post(handlers::proxy::image_info))
        .route("/api/image/download", post(handlers::proxy::image_download))
        .route("/api/build", post(handlers::build::trigger_build))
        .nest_service("/static", ServeDir::new(&static_dir))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = config
        .bind
        .parse()
        .with_context(|| format!("invalid bind address: {}", config.bind))?;

    info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {}", addr))?;
    axum::serve(listener, app).await.context("axum serve")?;

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}

fn static_dir_path() -> PathBuf {
    // When deployed, the binary is expected to sit next to a `static/` directory
    // (or the directory passed via AUTOIMAGE_STATIC_DIR). During `cargo run`
    // from the project root, this resolves to ./static.
    if let Ok(p) = std::env::var("AUTOIMAGE_STATIC_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from("static")
}
