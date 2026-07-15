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
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("autoimage/", env!("CARGO_PKG_VERSION")));

        // Allow HTTPS_PROXY (or HTTP_PROXY) to route all upstream traffic
        // (proxy.vvvv.ee, api.github.com, …) through a corporate proxy.
        // Internal hosts (127.0.0.1, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
        // bypass the proxy.
        //
        // The proxy URL may contain credentials — read from env, never logged.
        let proxy_url = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .ok();
        match proxy_url {
            Some(u) if !u.trim().is_empty() => {
                // Build the list of hosts that should bypass the proxy.
                // Defaults: loopback + RFC1918 private ranges.
                let mut no_proxy: Vec<String> = vec![
                    "127.0.0.1".into(),
                    "::1".into(),
                    "localhost".into(),
                    "10.0.0.0/8".into(),
                    "172.16.0.0/12".into(),
                    "192.168.0.0/16".into(),
                ];
                if let Ok(v) = std::env::var("NO_PROXY")
                    .or_else(|_| std::env::var("no_proxy"))
                {
                    for h in v.split(',') {
                        let h = h.trim();
                        if !h.is_empty() {
                            no_proxy.push(h.to_string());
                        }
                    }
                }

                let proxy = reqwest::Proxy::custom(move |url| {
                    let host = url.host_str().unwrap_or("").to_string();
                    if no_proxy.iter().any(|p| host_matches(&host, p)) {
                        tracing::debug!(%host, "internal host → direct");
                        return None;
                    }
                    tracing::debug!(%host, "external host → via proxy");
                    Some(u.clone())
                });
                builder = builder.proxy(proxy);
                tracing::info!("upstream HTTP proxy configured; internal hosts bypass");
            }
            _ => {
                tracing::info!("no HTTPS_PROXY/HTTP_PROXY set; upstream traffic goes direct");
            }
        }

        let http = builder.build().expect("reqwest client builds");
        Self {
            config: Arc::new(config),
            dockerfile_path: Arc::new(PathBuf::from("Dockerfile")),
            http: Arc::new(http),
        }
    }
}

fn url_host(s: &str) -> Option<String> {
    // minimal URL → host extraction, no URL crate needed
    let rest = s.trim().trim_start_matches("http://").trim_start_matches("https://");
    let host_end = rest.find('/').unwrap_or(rest.len());
    let host_port = &rest[..host_end];
    let host = host_port.rsplit_once(':').map(|(h, _)| h).unwrap_or(host_port);
    Some(host.to_ascii_lowercase())
}

fn host_matches(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pat = pattern.to_ascii_lowercase();
    if pat.contains('/') {
        // CIDR — only IPv4 supported for simplicity
        if let Some((base, bits)) = pat.split_once('/') {
            if let Ok(mask) = u32::from_str_radix(bits, 10) {
                if let Some(ip) = parse_ipv4(&host) {
                    if let Some(base_ip) = parse_ipv4(base) {
                        let m = if mask == 0 { 0u32 } else { !0u32 << (32 - mask) };
                        return (ip & m) == (base_ip & m);
                    }
                }
            }
        }
        return false;
    }
    if pat.starts_with('.') {
        // suffix: ".efoxconn.com" matches "x.efoxconn.com" and "efoxconn.com"
        host.ends_with(&pat) || host == &pat[1..]
    } else {
        host == pat
    }
}

fn parse_ipv4(s: &str) -> Option<u32> {
    let mut out = 0u32;
    let mut octets = s.split('.');
    for _ in 0..4 {
        let o = u32::from_str_radix(octets.next()?, 10).ok()?;
        if o > 255 {
            return None;
        }
        out = (out << 8) | o;
    }
    if octets.next().is_some() {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_host_extracts() {
        assert_eq!(url_host("https://proxy.vvvv.ee/path"), Some("proxy.vvvv.ee".into()));
        assert_eq!(url_host("http://example.com:8080"), Some("example.com".into()));
    }
    #[test]
    fn host_matches_suffix() {
        assert!(host_matches("api.github.com", ".github.com"));
        assert!(!host_matches("api.github.com", ".efoxconn.com"));
        assert!(host_matches("x.efoxconn.com", ".efoxconn.com"));
    }
    #[test]
    fn host_matches_cidr() {
        assert!(host_matches("10.0.0.5", "10.0.0.0/8"));
        assert!(host_matches("10.255.255.255", "10.0.0.0/8"));
        assert!(!host_matches("11.0.0.5", "10.0.0.0/8"));
        assert!(!host_matches("api.github.com", "10.0.0.0/8"));
        assert!(host_matches("192.168.1.1", "192.168.0.0/16"));
    }
}
