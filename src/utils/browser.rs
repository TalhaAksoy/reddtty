//! Safe external browser launching.
//!
//! URLs are validated and passed as a single process argument; no shell is
//! involved, so terminal content cannot inject commands.

use std::net::IpAddr;
use std::process::Command;

use url::Url;

/// Returns true for http(s) URLs that are safe to hand to the system browser.
pub fn is_safe_web_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    match url.host_str() {
        Some(host) => {
            let host = host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_ascii_lowercase();
            if host == "localhost" || host.ends_with(".localhost") {
                return false;
            }
            if let Ok(ip) = host.parse::<IpAddr>() {
                let private = match ip {
                    IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_broadcast(),
                    IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
                };
                return !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || private);
            }
            true
        }
        None => false,
    }
}

pub fn open_url(raw: &str) -> Result<(), String> {
    if !is_safe_web_url(raw) {
        return Err("Refusing to open an unsafe or malformed URL".to_string());
    }

    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(raw).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", raw]).spawn()
    } else {
        Command::new("xdg-open").arg(raw).spawn()
    };

    result
        .map(|_| ())
        .map_err(|e| format!("Could not open browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_urls() {
        assert!(is_safe_web_url("https://www.reddit.com/r/rust"));
        assert!(is_safe_web_url("http://example.com/a?b=c"));
    }

    #[test]
    fn rejects_unsafe_schemes_and_hosts() {
        assert!(!is_safe_web_url("file:///etc/passwd"));
        assert!(!is_safe_web_url("javascript:alert(1)"));
        assert!(!is_safe_web_url("https://localhost:8080/x"));
        assert!(!is_safe_web_url("https://127.0.0.1/"));
        assert!(!is_safe_web_url("http://10.0.0.1/"));
        assert!(!is_safe_web_url("http://192.168.1.1/"));
        assert!(!is_safe_web_url("http://[::1]/"));
        assert!(!is_safe_web_url("not a url"));
    }
}
