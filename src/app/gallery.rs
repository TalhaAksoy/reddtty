use crate::event::ViewId;
use crate::models::{MediaItem, Post};

#[derive(Clone, Debug)]
pub struct GalleryView {
    pub id: ViewId,
    pub post: Post,
    pub images: Vec<MediaItem>,
    pub index: usize,
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
        })
    }

    pub fn next(&mut self) {
        if !self.images.is_empty() {
            self.index = (self.index + 1) % self.images.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.images.is_empty() {
            self.index = (self.index + self.images.len() - 1) % self.images.len();
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
    fn no_images_means_no_viewer() {
        let mut p = post();
        p.content = PostContent::Text;
        assert!(GalleryView::new(ViewId(1), p).is_none());
    }
}
