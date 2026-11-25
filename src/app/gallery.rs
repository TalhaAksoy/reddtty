use crate::event::ViewId;
use crate::media::ImageView;
use crate::models::{MediaItem, Post};

#[derive(Clone, Debug)]
pub struct GalleryView {
    pub id: ViewId,
    pub post: Post,
    pub images: Vec<MediaItem>,
    pub index: usize,
    /// Zoom/pan for the current image. Reset when navigating.
    pub view: ImageView,
}

impl GalleryView {
    pub fn new(id: ViewId, post: Post) -> Option<Self> {
        let images = post.images();
        if images.is_empty() {
            return None;
        }
        Some(Self {
            id,
            post,
            images,
            index: 0,
            view: ImageView::default(),
        })
    }

    pub fn next(&mut self) {
        if !self.images.is_empty() {
            self.index = (self.index + 1) % self.images.len();
            self.reset_view();
        }
    }

    pub fn prev(&mut self) {
        if !self.images.is_empty() {
            self.index = (self.index + self.images.len() - 1) % self.images.len();
            self.reset_view();
        }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.images.len() {
            self.index = index;
            self.reset_view();
        }
    }

    pub const ZOOM_STEPS: [u16; 5] = [100, 150, 200, 300, 400];

    pub fn zoom_in(&mut self) {
        let index = Self::ZOOM_STEPS
            .iter()
            .position(|zoom| *zoom == self.view.zoom)
            .unwrap_or(0);
        self.view.zoom = Self::ZOOM_STEPS[(index + 1).min(Self::ZOOM_STEPS.len() - 1)];
    }

    pub fn zoom_out(&mut self) {
        let index = Self::ZOOM_STEPS
            .iter()
            .position(|zoom| *zoom == self.view.zoom)
            .unwrap_or(0);
        self.view.zoom = Self::ZOOM_STEPS[index.saturating_sub(1)];
        if self.view.zoom <= 100 {
            self.view.pan_x = 0;
            self.view.pan_y = 0;
        }
    }

    pub fn reset_view(&mut self) {
        self.view = ImageView::default();
    }

    pub fn pan(&mut self, dx: i16, dy: i16) {
        if self.view.zoom <= 100 {
            return;
        }
        self.view.pan_x = (self.view.pan_x + dx).clamp(-100, 100);
        self.view.pan_y = (self.view.pan_y + dy).clamp(-100, 100);
        if self.view.zoom >= 100 {
            // Panning is only meaningful while zoomed in.
            self.view.zoom = self.view.zoom.max(150);
        }
    }

    pub fn zoom_label(&self) -> String {
        format!("{}%", self.view.zoom)
    }

    pub fn pan_label(&self) -> String {
        if self.view.zoom <= 100 {
            "fit".into()
        } else {
            format!("x{:+} y{:+}", self.view.pan_x, self.view.pan_y)
        }
    }

    pub fn current(&self) -> Option<&MediaItem> {
        self.images.get(self.index)
    }

    pub fn position_label(&self) -> String {
        format!("{} / {}", self.index + 1, self.images.len().max(1))
    }

    pub fn title(&self) -> String {
        crate::utils::text::sanitize(&self.post.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PostContent;

    fn post() -> Post {
        Post {
            id: "g".into(),
            subreddit: "pics".into(),
            author: "a".into(),
            title: "Gallery".into(),
            body: String::new(),
            created_utc: 0,
            score: 0,
            num_comments: 0,
            permalink: "/r/pics/g".into(),
            content: PostContent::Gallery {
                items: vec![
                    MediaItem::new("a"),
                    MediaItem::new("b"),
                    MediaItem::new("c"),
                ],
            },
            flair: None,
            nsfw: false,
            spoiler: false,
            stickied: false,
            locked: false,
            removed: false,
            is_self: false,
            domain: None,
            thumbnail: None,
        }
    }

    #[test]
    fn navigation_wraps() {
        let mut g = GalleryView::new(ViewId(1), post()).unwrap();
        assert_eq!(g.current().unwrap().url, "a");
        g.prev();
        assert_eq!(g.current().unwrap().url, "c");
        g.next();
        assert_eq!(g.current().unwrap().url, "a");
        assert_eq!(g.position_label(), "1 / 3");
    }

    #[test]
    fn zoom_pan_and_navigation_reset_the_view() {
        let mut g = GalleryView::new(ViewId(1), post()).unwrap();
        g.zoom_in();
        assert_eq!(g.view.zoom, 150);
        g.zoom_in();
        assert_eq!(g.view.zoom, 200);
        g.pan(20, -10);
        assert_eq!(g.view.pan_x, 20);
        assert_eq!(g.view.pan_y, -10);
        g.next();
        assert_eq!(g.index, 1);
        assert!(g.view.is_base());
        g.zoom_in();
        g.zoom_out();
        assert_eq!(g.view.zoom, 100);
        assert_eq!(g.view.pan_x, 0);
        g.zoom_in();
        g.select(2);
        assert!(g.view.is_base());
        assert_eq!(g.index, 2);
    }

    #[test]
    fn no_images_means_no_viewer() {
        let mut p = post();
        p.content = PostContent::Text;
        assert!(GalleryView::new(ViewId(1), p).is_none());
    }
}
