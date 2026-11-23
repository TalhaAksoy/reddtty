use std::path::PathBuf;
use std::time::Duration;

use reqwest::Client;
use url::Url;

use crate::utils::browser::is_safe_web_url;

/// Download an image with hard limits on bytes and time. Local `file://` URLs
/// are supported because the demo provider stores its fixtures on disk.
pub async fn download_image(
    client: &Client,
    raw_url: &str,
    max_bytes: usize,
    local_root: Option<&std::path::Path>,
    allow_loopback: bool,
) -> Result<Vec<u8>, String> {
    if raw_url.trim().is_empty() {
        return Err("empty image URL".into());
    }

    if let Ok(url) = Url::parse(raw_url)
        && url.scheme() == "file"
    {
        let path: PathBuf = url
            .to_file_path()
            .map_err(|_| "invalid file URL".to_string())?;
        return read_allowed_local(&path, max_bytes, local_root).await;
    }

    // Allow plain local paths only when they exist and live under an allowed
    // root; never read arbitrary filesystem paths from remote content.
    let path = PathBuf::from(raw_url);
    if !raw_url.contains("://") && path.exists() {
        return read_allowed_local(&path, max_bytes, local_root).await;
    }

    if !is_safe_web_url(raw_url) {
        let loopback_ok = allow_loopback && is_loopback_url(raw_url);
        if !loopback_ok {
            return Err("refusing to fetch an unsafe image URL".into());
        }
    }

    let response = client
        .get(raw_url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "image download timed out".to_string()
            } else if e.is_connect() {
                "could not connect to the image host".to_string()
            } else {
                format!("image request failed: {e}")
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("image host returned HTTP {}", status.as_u16()));
    }
    if let Some(len) = response.content_length()
        && len > max_bytes as u64
    {
        return Err(format!(
            "image is too large ({} MB, limit {} MB)",
            len / 1_048_576,
            max_bytes / 1_048_576
        ));
    }

    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("image download failed: {e}"))?
    {
        if bytes.len() + chunk.len() > max_bytes {
            return Err(format!(
                "image exceeds the {} MB download limit",
                max_bytes / 1_048_576
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("image response was empty".into());
    }
    Ok(bytes)
}

fn is_loopback_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host == "127.0.0.1"
            || host == "::1"
            || host == "0.0.0.0"
            || host.ends_with(".localhost")
    })
}

async fn read_allowed_local(
    path: &std::path::Path,
    max_bytes: usize,
    local_root: Option<&std::path::Path>,
) -> Result<Vec<u8>, String> {
    let Some(root) = local_root else {
        return Err("local images are not allowed for this provider".into());
    };
    let root = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| "local image root is unavailable".to_string())?;
    let path = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| "local image could not be resolved".to_string())?;
    if !path.starts_with(&root) {
        return Err("local image path is outside the allowed cache directory".into());
    }
    read_local(&path, max_bytes).await
}

async fn read_local(path: &std::path::Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("could not read local image: {e}"))?;
    if meta.len() > max_bytes as u64 {
        return Err("local image exceeds the download limit".into());
    }
    tokio::fs::read(path)
        .await
        .map_err(|e| format!("could not read local image: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_local_files() {
        let dir = std::env::temp_dir().join(format!("reddtty-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.bin");
        std::fs::write(&path, b"hello").unwrap();
        let client = Client::new();
        let url = Url::from_file_path(&path).unwrap().to_string();
        let bytes = download_image(&client, &url, 1024, Some(&dir), false)
            .await
            .unwrap();
        assert_eq!(bytes, b"hello");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn rejects_oversize_local_files() {
        let dir = std::env::temp_dir().join(format!("reddtty-dl2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.bin");
        std::fs::write(&path, vec![0u8; 64]).unwrap();
        let client = Client::new();
        let url = Url::from_file_path(&path).unwrap().to_string();
        assert!(
            download_image(&client, &url, 10, Some(&dir), false)
                .await
                .is_err()
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn rejects_unsafe_urls() {
        let client = Client::new();
        assert!(
            download_image(&client, "file:///etc/passwd", 1024, None, false)
                .await
                .is_err()
        );
        assert!(
            download_image(&client, "https://127.0.0.1/x", 1024, None, false)
                .await
                .is_err()
        );
    }
}
