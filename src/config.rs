use std::env;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_proxy_base_url")]
    pub proxy_base_url: String,
    pub github: GithubConfig,
    pub target: TargetConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubConfig {
    pub owner: String,
    pub repo: String,
    pub workflow_file: String,
    #[serde(default = "default_default_branch")]
    pub default_branch: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    #[serde(default = "default_target_repo")]
    pub repo: String,
    #[serde(default)]
    pub namespace: Option<String>,
}

fn default_bind() -> String { "0.0.0.0:8080".to_string() }
fn default_proxy_base_url() -> String { "https://proxy.vvvv.ee".to_string() }
fn default_default_branch() -> String { "main".to_string() }
fn default_target_repo() -> String { "ghcr.io".to_string() }

impl Config {
    /// Load from `./config.toml` then overlay environment variables. Returns a
    /// complete Config (with defaults for any missing optional fields).
    pub fn load() -> Result<Self> {
        Self::load_from(Path::new(CONFIG_PATH))
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        // GH_TOKEN must come from env, never from a config file.
        // We do not look it up here; handlers read it directly via env::var.

        let mut cfg: Config = if path.exists() {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            // Defensive: refuse a config file that tries to set GH_TOKEN
            if text.lines().any(|l| l.trim_start().starts_with("gh_token")) {
                return Err(anyhow!("GH_TOKEN must be set via the environment, not config.toml"));
            }
            toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?
        } else {
            // Synthesize a minimal config from env / defaults.
            let owner = env::var("GH_OWNER").unwrap_or_default();
            let repo = env::var("GH_REPO").unwrap_or_default();
            let workflow = env::var("GH_WORKFLOW").unwrap_or_else(|_| "build.yml".to_string());
            Config {
                bind: default_bind(),
                proxy_base_url: default_proxy_base_url(),
                github: GithubConfig {
                    owner,
                    repo,
                    workflow_file: workflow,
                    default_branch: default_default_branch(),
                },
                target: TargetConfig {
                    repo: default_target_repo(),
                    namespace: None,
                },
            }
        };

        cfg.apply_env_overrides();
        cfg.fill_namespace_default();
        Ok(cfg)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(v) = env::var("APP_BIND") { self.bind = v; }
        if let Ok(v) = env::var("PROXY_BASE_URL") { self.proxy_base_url = v; }
        if let Ok(v) = env::var("GH_OWNER") { self.github.owner = v; }
        if let Ok(v) = env::var("GH_REPO") { self.github.repo = v; }
        if let Ok(v) = env::var("GH_WORKFLOW") { self.github.workflow_file = v; }
        if let Ok(v) = env::var("GH_DEFAULT_BRANCH") { self.github.default_branch = v; }
        if let Ok(v) = env::var("TARGET_REPO") { self.target.repo = v; }
        if let Ok(v) = env::var("TARGET_NAMESPACE") {
            self.target.namespace = Some(v);
        }
    }

    fn fill_namespace_default(&mut self) {
        if self.target.namespace.is_none() {
            self.target.namespace = Some(self.github.owner.clone());
        }
    }

    pub fn namespace(&self) -> &str {
        self.target.namespace.as_deref().unwrap_or(&self.github.owner)
    }
}
