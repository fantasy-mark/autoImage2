use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub dockerfile_path: Arc<PathBuf>,
    pub http: Arc<reqwest::Client>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("autoimage/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds");
        Self {
            config: Arc::new(config),
            dockerfile_path: Arc::new(PathBuf::from("Dockerfile")),
            http: Arc::new(http),
        }
    }
}
