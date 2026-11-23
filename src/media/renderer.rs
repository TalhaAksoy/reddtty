//! Asynchronous image pipeline. Downloads, decoding and protocol encoding all
//! happen away from the UI thread; the UI only ever renders a cached protocol.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use ratatui::layout::Size;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Resize};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{ImageProtocolSetting, Settings};
use crate::event::AppEvent;

use super::cache::{ImageCache, ImageKey};
use super::downloader::download_image;

pub enum ImageStatus<'a> {
    Disabled,
    NotRequested,
    Loading,
    Ready(&'a Protocol),
    Failed(&'a str),
}

pub struct ImageManager {
    picker: Picker,
    cache: ImageCache,
    pending: HashSet<ImageKey>,
    failures: HashMap<ImageKey, String>,
    tx: UnboundedSender<AppEvent>,
    http: reqwest::Client,
    enabled: bool,
    max_bytes: usize,
    local_root: Option<PathBuf>,
    limiter: Arc<Semaphore>,
    protocol_label: String,
}

impl ImageManager {
    pub fn new(picker: Picker, settings: &Settings, tx: UnboundedSender<AppEvent>) -> Self {
        let enabled =
            settings.images.enabled && settings.images.protocol != ImageProtocolSetting::Disabled;
        let built = reqwest::Client::builder()
            .user_agent(settings.reddit.user_agent.clone())
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many image redirects")
                } else if crate::utils::browser::is_safe_web_url(attempt.url().as_str()) {
                    attempt.follow()
                } else {
                    attempt.error("image redirect pointed at a private or unsafe host")
                }
            }))
            .build();
        let (http, client_ok) = match built {
            Ok(client) => (client, true),
            Err(error) => {
                tracing::warn!(error = %error, "could not build the image HTTP client");
                (reqwest::Client::new(), false)
            }
        };
        let enabled = enabled && client_ok;
        let protocol_label = format!("{:?}", picker.protocol_type());
        Self {
            picker,
            cache: ImageCache::new((settings.images.max_cache_mb as usize) * 1024 * 1024),
            pending: HashSet::new(),
            failures: HashMap::new(),
            tx,
            http,
            enabled,
            max_bytes: (settings.images.max_download_mb as usize) * 1024 * 1024,
            local_root: Some(Settings::cache_dir()),
            limiter: Arc::new(Semaphore::new(3)),
            protocol_label,
        }
    }

    #[cfg(test)]
    pub fn disabled(tx: UnboundedSender<AppEvent>) -> Self {
        let settings = Settings::default();
        let mut manager = Self::new(Picker::halfblocks(), &settings, tx);
        manager.enabled = false;
        manager
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn protocol_label(&self) -> &str {
        &self.protocol_label
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn animating(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn cache_used_bytes(&self) -> usize {
        self.cache.used_bytes()
    }

    pub fn clear_failures(&mut self) {
        self.failures.clear();
    }

    /// Queue a download/encode if this exact (url, size) is not already known.
    /// `original_url` keys the cache; `fetch_url` is what gets fetched (the
    /// demo provider resolves `demo://` URLs to local files).
    pub fn request(&mut self, original_url: &str, fetch_url: &str, width: u16, height: u16) {
        if !self.enabled || width == 0 || height == 0 || original_url.trim().is_empty() {
            return;
        }
        if self.pending.len() >= 64 {
            // A fast scroll should not queue thousands of downloads. Visible
            // images will be requested again on the next frame.
            return;
        }
        let key = ImageKey::new(original_url, width, height);
        if self.cache.contains(&key)
            || self.pending.contains(&key)
            || self.failures.contains_key(&key)
        {
            return;
        }
        self.pending.insert(key.clone());

        let picker = self.picker.clone();
        let http = self.http.clone();
        let tx = self.tx.clone();
        let limiter = self.limiter.clone();
        let max_bytes = self.max_bytes;
        let local_root = self.local_root.clone();
        let fetch_url = fetch_url.to_string();

        tokio::spawn(async move {
            let Ok(_permit) = limiter.acquire_owned().await else {
                let _ = tx.send(AppEvent::ImageReady {
                    key,
                    result: Err("image worker queue is closed".into()),
                });
                return;
            };
            let result =
                match download_image(&http, &fetch_url, max_bytes, local_root.as_deref(), false)
                    .await
                {
                    Ok(bytes) => {
                        match tokio::task::spawn_blocking(move || {
                            decode_and_encode(&picker, bytes, width, height)
                        })
                        .await
                        {
                            Ok(inner) => inner,
                            Err(err) => Err(format!("image worker failed: {err}")),
                        }
                    }
                    Err(err) => Err(err),
                };
            let _ = tx.send(AppEvent::ImageReady { key, result });
        });
    }

    pub fn poll(&mut self, url: &str, width: u16, height: u16) -> ImageStatus<'_> {
        if !self.enabled {
            return ImageStatus::Disabled;
        }
        let key = ImageKey::new(url, width, height);
        if let Some(protocol) = self.cache.get(&key) {
            return ImageStatus::Ready(protocol);
        }
        if let Some(error) = self.failures.get(&key) {
            return ImageStatus::Failed(error);
        }
        if self.pending.contains(&key) {
            return ImageStatus::Loading;
        }
        ImageStatus::NotRequested
    }

    pub fn on_result(&mut self, key: ImageKey, result: Result<Protocol, String>) {
        self.pending.remove(&key);
        match result {
            Ok(protocol) => {
                self.failures.remove(&key);
                self.cache.insert(key, protocol);
            }
            Err(error) => {
                if self.failures.len() > 256 {
                    self.failures.clear();
                }
                self.failures.insert(key, error);
            }
        }
    }
}

fn decode_and_encode(
    picker: &Picker,
    bytes: Vec<u8>,
    width: u16,
    height: u16,
) -> Result<Protocol, String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("could not inspect image data: {e}"))?;
    let format = reader
        .format()
        .ok_or_else(|| "unknown image format".to_string())?;
    if !matches!(
        format,
        image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::Gif
            | image::ImageFormat::WebP
    ) {
        return Err(format!("unsupported image format: {format:?}"));
    }

    let mut reader = reader;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8_000);
    limits.max_image_height = Some(8_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);

    let mut img = reader
        .decode()
        .map_err(|e| format!("could not decode image: {e}"))?;
    if img.width() > 4_096 || img.height() > 4_096 {
        img = img.thumbnail(4_096, 4_096);
    }
    picker
        .new_protocol(
            img,
            Size::new(width, height),
            Resize::Scale(Some(FilterType::Triangle)),
        )
        .map_err(|e| format!("could not encode image for the terminal: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes() -> Vec<u8> {
        let dir = std::env::temp_dir().join(format!("reddtty-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.png");
        let img = image::RgbImage::from_fn(8, 8, |x, y| {
            image::Rgb([(x * 20) as u8, (y * 20) as u8, 120])
        });
        img.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
        bytes
    }

    #[test]
    fn decodes_png_into_a_protocol() {
        let picker = Picker::halfblocks();
        let protocol = decode_and_encode(&picker, png_bytes(), 8, 4).unwrap();
        assert!(protocol.size().width > 0);
        assert!(protocol.size().height > 0);
    }

    #[test]
    fn rejects_garbage_bytes() {
        let picker = Picker::halfblocks();
        assert!(decode_and_encode(&picker, b"not an image".to_vec(), 8, 4).is_err());
    }

    #[test]
    fn rejects_unsupported_formats() {
        let picker = Picker::halfblocks();
        let mut bmp = vec![0u8; 64];
        bmp[0] = b'B';
        bmp[1] = b'M';
        assert!(decode_and_encode(&picker, bmp, 8, 4).is_err());
    }

    #[test]
    fn tiny_and_wide_images_both_encode() {
        let picker = Picker::halfblocks();
        let tiny = image::DynamicImage::new_rgb8(1, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        tiny.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let protocol = decode_and_encode(&picker, bytes.into_inner(), 4, 2).unwrap();
        assert!(protocol.size().width > 0);

        let wide = image::DynamicImage::new_rgb8(4200, 40);
        let mut bytes = std::io::Cursor::new(Vec::new());
        wide.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let protocol = decode_and_encode(&picker, bytes.into_inner(), 60, 5).unwrap();
        assert!(protocol.size().width <= 60 + 1);
    }

    #[test]
    fn jpeg_is_decoded() {
        let picker = Picker::halfblocks();
        let img = image::DynamicImage::new_rgb8(32, 24);
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Jpeg).unwrap();
        assert!(decode_and_encode(&picker, bytes.into_inner(), 12, 8).is_ok());
    }

    #[test]
    fn disabled_manager_reports_disabled() {
        use crate::config::Settings;
        use tokio::sync::mpsc::unbounded_channel;
        let mut settings = Settings::default();
        settings.images.enabled = false;
        let (tx, _rx) = unbounded_channel();
        let mut manager = ImageManager::new(Picker::halfblocks(), &settings, tx);
        assert!(matches!(
            manager.poll("demo://x", 10, 5),
            ImageStatus::Disabled
        ));
        assert!(!manager.enabled());
    }
}
