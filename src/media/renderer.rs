//! Asynchronous image pipeline. Downloads, decoding and protocol encoding all
//! happen away from the UI thread; the UI only ever renders a cached protocol.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ratatui::layout::Size;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Resize};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{ImageProtocolSetting, Settings};
use crate::event::AppEvent;

use super::cache::{ImageCache, ImageKey, ImageView};
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
    client_ok: bool,
    auto_protocol: ProtocolType,
    max_bytes: usize,
    local_root: Option<PathBuf>,
    source_cache: Arc<Mutex<SourceCache>>,
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
        let auto_protocol = picker.protocol_type();
        let protocol_label = format!("{:?}", picker.protocol_type());
        Self {
            picker,
            cache: ImageCache::new((settings.images.max_cache_mb as usize) * 1024 * 1024),
            pending: HashSet::new(),
            failures: HashMap::new(),
            tx,
            http,
            enabled,
            client_ok,
            auto_protocol,
            max_bytes: (settings.images.max_download_mb as usize) * 1024 * 1024,
            local_root: Some(Settings::cache_dir()),
            source_cache: Arc::new(Mutex::new(SourceCache::new(64 * 1024 * 1024))),
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

    /// Switch protocol at runtime. Cached protocols are cleared because they
    /// were encoded for the previous terminal protocol.
    pub fn set_protocol(
        &mut self,
        setting: ImageProtocolSetting,
        images_enabled: bool,
    ) -> Result<(), String> {
        let protocol = match setting {
            ImageProtocolSetting::Auto => self.auto_protocol,
            ImageProtocolSetting::Kitty => ProtocolType::Kitty,
            ImageProtocolSetting::Iterm2 => ProtocolType::Iterm2,
            ImageProtocolSetting::Sixel => ProtocolType::Sixel,
            ImageProtocolSetting::Halfblocks => ProtocolType::Halfblocks,
            ImageProtocolSetting::Disabled => {
                self.enabled = false;
                self.cache.clear();
                self.pending.clear();
                self.failures.clear();
                self.protocol_label = "Disabled".into();
                return Ok(());
            }
        };
        self.picker.set_protocol_type(protocol);
        self.cache.clear();
        self.pending.clear();
        self.failures.clear();
        self.enabled = images_enabled && self.client_ok;
        if !images_enabled {
            self.protocol_label = "Disabled".into();
        } else {
            self.protocol_label = format!("{protocol:?}");
        }
        Ok(())
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
        self.request_view(original_url, fetch_url, width, height, ImageView::default());
    }

    /// Like [`request`](Self::request), but for a zoomed/panned view.
    pub fn request_view(
        &mut self,
        original_url: &str,
        fetch_url: &str,
        width: u16,
        height: u16,
        view: ImageView,
    ) {
        if !self.enabled || width == 0 || height == 0 || original_url.trim().is_empty() {
            return;
        }
        if self.pending.len() >= 64 {
            // A fast scroll should not queue thousands of downloads. Visible
            // images will be requested again on the next frame.
            return;
        }
        let key = ImageKey::with_view(original_url, width, height, view);
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
        let source_cache = self.source_cache.clone();
        let fetch_url = fetch_url.to_string();

        tokio::spawn(async move {
            let Ok(_permit) = limiter.acquire_owned().await else {
                let _ = tx.send(AppEvent::ImageReady {
                    key,
                    result: Err("image worker queue is closed".into()),
                });
                return;
            };
            let cached = source_cache
                .lock()
                .ok()
                .and_then(|mut cache| cache.get(&fetch_url));
            let bytes = match cached {
                Some(bytes) => Ok(bytes),
                None => download_image(&http, &fetch_url, max_bytes, local_root.as_deref(), false)
                    .await
                    .map(|bytes| {
                        let bytes = Arc::new(bytes);
                        if let Ok(mut cache) = source_cache.lock() {
                            cache.insert(fetch_url.clone(), bytes.clone());
                        }
                        bytes
                    }),
            };
            let result = match bytes {
                Ok(bytes) => {
                    match tokio::task::spawn_blocking(move || {
                        decode_and_encode(&picker, bytes.as_slice(), width, height, view)
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
        self.poll_view(url, width, height, ImageView::default())
    }

    pub fn poll_view(
        &mut self,
        url: &str,
        width: u16,
        height: u16,
        view: ImageView,
    ) -> ImageStatus<'_> {
        if !self.enabled {
            return ImageStatus::Disabled;
        }
        let key = ImageKey::with_view(url, width, height, view);
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
        // A protocol switch clears pending work; late results from the previous
        // protocol must not enter the new cache.
        if !self.pending.remove(&key) {
            return;
        }
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

struct SourceCache {
    entries: HashMap<String, Arc<Vec<u8>>>,
    order: VecDeque<String>,
    used: usize,
    max: usize,
}

impl SourceCache {
    fn new(max: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            used: 0,
            max,
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let value = self.entries.get(key).cloned()?;
        self.order.retain(|existing| existing != key);
        self.order.push_back(key.to_string());
        Some(value)
    }

    fn insert(&mut self, key: String, bytes: Arc<Vec<u8>>) {
        if let Some(old) = self.entries.remove(&key) {
            self.used = self.used.saturating_sub(old.len());
            self.order.retain(|existing| existing != &key);
        }
        self.used = self.used.saturating_add(bytes.len());
        self.entries.insert(key.clone(), bytes);
        self.order.push_back(key);
        while self.used > self.max {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(old) = self.entries.remove(&oldest) {
                self.used = self.used.saturating_sub(old.len());
            }
        }
    }
}

fn decode_and_encode(
    picker: &Picker,
    bytes: &[u8],
    width: u16,
    height: u16,
    view: ImageView,
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
    let img = crop_for_view(img, picker, width, height, view);
    picker
        .new_protocol(
            img,
            Size::new(width, height),
            Resize::Scale(Some(FilterType::Triangle)),
        )
        .map_err(|e| format!("could not encode image for the terminal: {e}"))
}

/// Crop a centered viewport for zoom/pan. The crop uses the target's pixel
/// aspect ratio (cell size × picker font size) so `Resize::Scale` fills the
/// viewer without stretching.
fn crop_for_view(
    img: image::DynamicImage,
    picker: &Picker,
    width: u16,
    height: u16,
    view: ImageView,
) -> image::DynamicImage {
    if view.is_base() || width == 0 || height == 0 {
        return img;
    }
    let (source_w, source_h) = (img.width() as f32, img.height() as f32);
    if source_w < 2.0 || source_h < 2.0 {
        return img;
    }
    let font = picker.font_size();
    let cell_aspect = if font.width == 0 || font.height == 0 {
        width as f32 / height.max(1) as f32
    } else {
        (width as f32 * font.width as f32) / (height as f32 * font.height as f32)
    };
    if !cell_aspect.is_finite() || cell_aspect <= 0.0 {
        return img;
    }
    let zoom = (view.zoom.max(100) as f32) / 100.0;
    let mut crop_w = source_w / zoom;
    let mut crop_h = source_h / zoom;
    if crop_w / crop_h > cell_aspect {
        crop_w = crop_h * cell_aspect;
    } else {
        crop_h = crop_w / cell_aspect;
    }
    crop_w = crop_w.clamp(1.0, source_w);
    crop_h = crop_h.clamp(1.0, source_h);
    let slack_x = source_w - crop_w;
    let slack_y = source_h - crop_h;
    let pan_x = view.pan_x.clamp(-100, 100) as f32 / 100.0;
    let pan_y = view.pan_y.clamp(-100, 100) as f32 / 100.0;
    let center_x = source_w / 2.0 + pan_x * slack_x / 2.0;
    let center_y = source_h / 2.0 + pan_y * slack_y / 2.0;
    let x = (center_x - crop_w / 2.0).clamp(0.0, slack_x).round() as u32;
    let y = (center_y - crop_h / 2.0).clamp(0.0, slack_y).round() as u32;
    img.crop_imm(x, y, crop_w.round() as u32, crop_h.round() as u32)
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
        let protocol =
            decode_and_encode(&picker, &png_bytes(), 8, 4, ImageView::default()).unwrap();
        assert!(protocol.size().width > 0);
        assert!(protocol.size().height > 0);
    }

    #[test]
    fn rejects_garbage_bytes() {
        let picker = Picker::halfblocks();
        assert!(decode_and_encode(&picker, b"not an image", 8, 4, ImageView::default()).is_err());
    }

    #[test]
    fn rejects_unsupported_formats() {
        let picker = Picker::halfblocks();
        let mut bmp = vec![0u8; 64];
        bmp[0] = b'B';
        bmp[1] = b'M';
        assert!(decode_and_encode(&picker, &bmp, 8, 4, ImageView::default()).is_err());
    }

    #[test]
    fn source_bytes_cache_reuses_and_evicts() {
        let mut cache = SourceCache::new(9);
        cache.insert("a".into(), Arc::new(vec![1, 2, 3, 4]));
        cache.insert("b".into(), Arc::new(vec![5, 6, 7, 8]));
        assert!(cache.get("a").is_some());
        cache.insert("c".into(), Arc::new(vec![9; 4]));
        assert!(cache.get("a").is_some(), "recently used entry stays");
        assert!(
            cache.get("b").is_none(),
            "least-recently used entry evicted"
        );
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn zoom_and_pan_crop_the_source_image() {
        let image = image::DynamicImage::new_rgb8(200, 100);
        let picker = Picker::halfblocks();
        let base = crop_for_view(image.clone(), &picker, 20, 10, ImageView::default());
        assert_eq!((base.width(), base.height()), (200, 100));

        let zoomed = crop_for_view(
            image.clone(),
            &picker,
            20,
            10,
            ImageView {
                zoom: 200,
                pan_x: 0,
                pan_y: 0,
            },
        );
        assert!(zoomed.width() < 200 || zoomed.height() < 100);
        assert!(zoomed.width() > 0 && zoomed.height() > 0);

        let panned = crop_for_view(
            image,
            &picker,
            20,
            10,
            ImageView {
                zoom: 200,
                pan_x: 100,
                pan_y: -100,
            },
        );
        assert!(panned.width() > 0 && panned.height() > 0);
    }

    #[test]
    fn tiny_and_wide_images_both_encode() {
        let picker = Picker::halfblocks();
        let tiny = image::DynamicImage::new_rgb8(1, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        tiny.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let protocol =
            decode_and_encode(&picker, &bytes.into_inner(), 4, 2, ImageView::default()).unwrap();
        assert!(protocol.size().width > 0);

        let wide = image::DynamicImage::new_rgb8(4200, 40);
        let mut bytes = std::io::Cursor::new(Vec::new());
        wide.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let protocol =
            decode_and_encode(&picker, &bytes.into_inner(), 60, 5, ImageView::default()).unwrap();
        assert!(protocol.size().width <= 60 + 1);
    }

    #[test]
    fn jpeg_is_decoded() {
        let picker = Picker::halfblocks();
        let img = image::DynamicImage::new_rgb8(32, 24);
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Jpeg).unwrap();
        assert!(
            decode_and_encode(&picker, &bytes.into_inner(), 12, 8, ImageView::default()).is_ok()
        );
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
