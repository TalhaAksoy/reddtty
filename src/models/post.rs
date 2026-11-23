use serde::{Deserialize, Serialize};

/// A single media item (gallery image, preview, video thumbnail).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaItem {
    pub url: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl MediaItem {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            width: None,
            height: None,
        }
    }

    pub fn with_size(url: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            url: url.into(),
            width: Some(width),
            height: Some(height),
        }
    }
}

/// The kind of content a post holds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PostContent {
    Text,
    Image {
        url: String,
    },
    Gallery {
        items: Vec<MediaItem>,
    },
    Link {
        url: String,
        #[serde(default)]
        thumbnail: Option<String>,
    },
    Video {
        url: String,
        #[serde(default)]
        thumbnail: Option<String>,
        #[serde(default)]
        duration: Option<u32>,
    },
    Removed,
}

impl PostContent {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Image { .. } => "Image",
            Self::Gallery { .. } => "Gallery",
            Self::Link { .. } => "Link",
            Self::Video { .. } => "Video",
            Self::Removed => "Removed",
        }
    }
}

/// A Reddit post, normalised away from Reddit's JSON shape.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Post {
    pub id: String,
    pub subreddit: String,
    pub author: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub created_utc: i64,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub num_comments: u32,
    #[serde(default)]
    pub permalink: String,
    pub content: PostContent,
    #[serde(default)]
    pub flair: Option<String>,
    #[serde(default)]
    pub nsfw: bool,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default)]
    pub stickied: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub removed: bool,
    #[serde(default)]
    pub is_self: bool,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
}

impl Post {
    /// URL used for the in-post image preview, if any.
    pub fn preview_image(&self) -> Option<&str> {
        match &self.content {
            PostContent::Image { url } => Some(url),
            PostContent::Gallery { items } => items.first().map(|m| m.url.as_str()),
            PostContent::Link { thumbnail, .. } | PostContent::Video { thumbnail, .. } => {
                thumbnail.as_deref().filter(|u| !u.is_empty())
            }
            _ => self.thumbnail.as_deref().filter(|u| !u.is_empty()),
        }
    }

    /// All images that can be browsed in the gallery viewer.
    pub fn images(&self) -> Vec<MediaItem> {
        match &self.content {
            PostContent::Image { url } => vec![MediaItem::new(url.clone())],
            PostContent::Gallery { items } => items.clone(),
            PostContent::Link { thumbnail, .. } | PostContent::Video { thumbnail, .. } => {
                thumbnail.iter().cloned().map(MediaItem::new).collect()
            }
            _ => Vec::new(),
        }
    }

    pub fn has_gallery(&self) -> bool {
        matches!(&self.content, PostContent::Gallery { items } if items.len() > 1)
    }

    /// The best target for "open in browser".
    pub fn open_url(&self) -> Option<String> {
        match &self.content {
            PostContent::Link { url, .. } | PostContent::Video { url, .. } => Some(url.clone()),
            PostContent::Image { url } => Some(url.clone()),
            PostContent::Gallery { items } => items.first().map(|m| m.url.clone()),
            PostContent::Text => self.reddit_url(),
            PostContent::Removed => self.reddit_url(),
        }
    }

    pub fn reddit_url(&self) -> Option<String> {
        if self.permalink.is_empty() {
            return None;
        }
        if self.permalink.starts_with("http") {
            return Some(self.permalink.clone());
        }
        let path = if self.permalink.starts_with('/') {
            self.permalink.clone()
        } else {
            format!("/{}", self.permalink)
        };
        Some(format!("https://www.reddit.com{path}"))
    }

    pub fn is_hidden(&self, show_nsfw: bool, show_spoilers: bool) -> bool {
        (self.nsfw && !show_nsfw) || (self.spoiler && !show_spoilers)
    }

    pub fn hidden_reason(&self) -> Option<&'static str> {
        if self.nsfw {
            Some("NSFW content hidden")
        } else if self.spoiler {
            Some("Spoiler hidden")
        } else if self.removed {
            Some("This post was removed")
        } else {
            None
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.removed
            || self.title == "[deleted]"
            || self.title == "[removed]"
            || matches!(self.content, PostContent::Removed)
    }

    pub fn domain_or_subreddit(&self) -> &str {
        self.domain
            .as_deref()
            .filter(|d| !d.is_empty())
            .unwrap_or(&self.subreddit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_post(content: PostContent) -> Post {
        Post {
            id: "abc".into(),
            subreddit: "rust".into(),
            author: "alice".into(),
            title: "Hello".into(),
            body: String::new(),
            created_utc: 0,
            score: 10,
            num_comments: 2,
            permalink: "/r/rust/comments/abc/hello/".into(),
            content,
            flair: None,
            nsfw: false,
            spoiler: false,
            stickied: false,
            locked: false,
            removed: false,
            is_self: true,
            domain: None,
            thumbnail: None,
        }
    }

    #[test]
    fn preview_image_for_each_kind() {
        assert_eq!(
            base_post(PostContent::Image {
                url: "demo://img/a.png".into()
            })
            .preview_image(),
            Some("demo://img/a.png")
        );
        assert_eq!(
            base_post(PostContent::Gallery {
                items: vec![MediaItem::new("a"), MediaItem::new("b"),]
            })
            .preview_image(),
            Some("a")
        );
        assert_eq!(
            base_post(PostContent::Link {
                url: "https://example.com".into(),
                thumbnail: Some("t.png".into())
            })
            .preview_image(),
            Some("t.png")
        );
        assert_eq!(base_post(PostContent::Text).preview_image(), None);
    }

    #[test]
    fn reddit_url_is_absolute() {
        let post = base_post(PostContent::Text);
        assert_eq!(
            post.reddit_url().unwrap(),
            "https://www.reddit.com/r/rust/comments/abc/hello/"
        );
    }

    #[test]
    fn hidden_logic() {
        let mut post = base_post(PostContent::Text);
        post.nsfw = true;
        assert!(post.is_hidden(false, true));
        assert!(!post.is_hidden(true, true));
    }

    #[test]
    fn removed_posts_are_deleted() {
        let mut post = base_post(PostContent::Removed);
        post.removed = true;
        assert!(post.is_deleted());
    }

    #[test]
    fn deleted_author_does_not_hide_a_valid_post() {
        let mut post = base_post(PostContent::Text);
        post.author = "[deleted]".into();
        post.body = "still readable".into();
        assert!(!post.is_deleted());
    }
}
