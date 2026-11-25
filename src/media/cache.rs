use lru::LruCache;
use ratatui_image::protocol::Protocol;

/// A cropped/zoomed view of an image. `zoom` is a percentage where 100 means
/// the whole image fitted to the target; 200 shows half the width/height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageView {
    pub zoom: u16,
    pub pan_x: i16,
    pub pan_y: i16,
}

impl Default for ImageView {
    fn default() -> Self {
        Self {
            zoom: 100,
            pan_x: 0,
            pan_y: 0,
        }
    }
}

impl ImageView {
    pub fn is_base(self) -> bool {
        self.zoom <= 100 && self.pan_x == 0 && self.pan_y == 0
    }
}

/// Cache key: a URL plus the cell budget and crop it was encoded for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub url: String,
    pub width: u16,
    pub height: u16,
    pub view: ImageView,
}

impl ImageKey {
    pub fn new(url: impl Into<String>, width: u16, height: u16) -> Self {
        Self {
            url: url.into(),
            width,
            height,
            view: ImageView::default(),
        }
    }

    pub fn with_view(url: impl Into<String>, width: u16, height: u16, view: ImageView) -> Self {
        Self {
            url: url.into(),
            width,
            height,
            view,
        }
    }
}

pub struct CachedImage {
    pub protocol: Protocol,
    estimated_bytes: usize,
}

/// Memory-only LRU image cache. Protocols are encoded off the UI thread; this
/// structure only decides what stays resident.
pub struct ImageCache {
    inner: LruCache<ImageKey, CachedImage>,
    used_bytes: usize,
    max_bytes: usize,
}

impl ImageCache {
    pub fn new(max_bytes: usize) -> Self {
        let max_bytes = max_bytes.max(8 * 1024 * 1024);
        Self {
            inner: LruCache::unbounded(),
            used_bytes: 0,
            max_bytes,
        }
    }

    pub fn get(&mut self, key: &ImageKey) -> Option<&Protocol> {
        self.inner.get(key).map(|item| &item.protocol)
    }

    pub fn contains(&self, key: &ImageKey) -> bool {
        self.inner.contains(key)
    }

    pub fn insert(&mut self, key: ImageKey, protocol: Protocol) {
        let estimated = estimate_bytes(key.width, key.height);
        if let Some(old) = self.inner.put(
            key,
            CachedImage {
                protocol,
                estimated_bytes: estimated,
            },
        ) {
            self.used_bytes = self.used_bytes.saturating_sub(old.estimated_bytes);
        }
        self.used_bytes += estimated;
        self.evict();
    }

    fn evict(&mut self) {
        while self.used_bytes > self.max_bytes {
            let Some((_, item)) = self.inner.pop_lru() else {
                break;
            };
            self.used_bytes = self.used_bytes.saturating_sub(item.estimated_bytes);
        }
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
        self.used_bytes = 0;
    }
}

fn estimate_bytes(width: u16, height: u16) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(24)
        .max(32 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Size;
    use ratatui_image::Resize;
    use ratatui_image::picker::Picker;

    fn protocol() -> Protocol {
        let picker = Picker::halfblocks();
        let img = image::DynamicImage::new_rgb8(16, 16);
        picker
            .new_protocol(img, Size::new(16, 8), Resize::Fit(None))
            .unwrap()
    }

    #[test]
    fn insert_and_get() {
        let mut cache = ImageCache::new(8 * 1024 * 1024);
        let key = ImageKey::new("demo", 16, 8);
        cache.insert(key.clone(), protocol());
        assert!(cache.contains(&key));
        assert!(cache.get(&key).is_some());
        assert!(cache.used_bytes() > 0);
    }

    #[test]
    fn lru_evicts_oldest_when_over_budget() {
        let mut cache = ImageCache::new(8 * 1024 * 1024);
        // Force a tiny budget by inserting many distinct keys.
        for i in 0..400 {
            cache.insert(ImageKey::new(format!("img{i}"), 64, 32), protocol());
        }
        assert!(cache.len() < 400);
        assert!(cache.used_bytes() <= cache.max_bytes());
        assert!(!cache.contains(&ImageKey::new("img0", 64, 32)));
    }

    #[test]
    fn zoom_and_pan_views_use_distinct_cache_keys() {
        let base = ImageKey::new("img", 20, 10);
        let zoom = ImageKey::with_view(
            "img",
            20,
            10,
            ImageView {
                zoom: 200,
                pan_x: 0,
                pan_y: 0,
            },
        );
        let pan = ImageKey::with_view(
            "img",
            20,
            10,
            ImageView {
                zoom: 200,
                pan_x: 10,
                pan_y: 0,
            },
        );
        assert_ne!(base, zoom);
        assert_ne!(zoom, pan);
        assert!(base.view.is_base());
        assert!(!zoom.view.is_base());
    }

    #[test]
    fn clear_resets_memory() {
        let mut cache = ImageCache::new(8 * 1024 * 1024);
        cache.insert(ImageKey::new("a", 10, 10), protocol());
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.used_bytes(), 0);
    }
}
