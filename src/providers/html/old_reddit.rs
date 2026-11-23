//! Old Reddit (`old.reddit.com`) HTML provider.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use scraper::{ElementRef, Html};
use url::Url;

use crate::error::ProviderError;
use crate::models::{
    Comment, CommentSort, FeedPage, FeedRequest, FeedSort, FeedSpec, MediaItem, Post, PostContent,
    Subreddit,
};
use crate::providers::traits::{ContentProvider, ProviderCapabilities};

use super::client::{HtmlClient, HtmlClientConfig};
use super::parser::{
    absolute_url, all_in, attr, detect_gate, direct_child, direct_children, first_in,
    has_any_class, has_class, html_to_text, is_gallery_url, is_image_url, is_video_url,
    meta_content, next_page_url, parse_count, parse_timestamp, selector, structure_error, text,
};
use super::{HtmlState, comments_url, listing_url, sort_options};

pub struct OldRedditProvider {
    client: HtmlClient,
    state: HtmlState,
}

impl OldRedditProvider {
    pub fn new(config: HtmlClientConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            client: HtmlClient::new(&config)?,
            state: HtmlState::default(),
        })
    }

    fn parse_feed(&self, doc: &Html, page: &Url, now: i64) -> Result<Vec<Post>, ProviderError> {
        let listing_present = doc
            .select(&selector(
                "#siteTable, div.sitetable.linklisting, div.linklisting, div.thing.link",
            ))
            .next()
            .is_some();
        if !listing_present {
            if let Some(gate) = detect_gate(doc) {
                return Err(gate);
            }
            return Err(structure_error(
                "Old Reddit",
                "no post listing (#siteTable / .linklisting) was found",
            ));
        }

        let mut posts = Vec::new();
        let mut seen = HashSet::new();
        let mut promoted_skipped = 0usize;
        for element in doc.select(&selector("div.thing")) {
            if has_class(element, "comment") || has_class(element, "morechildren") {
                continue;
            }
            if has_any_class(element, &["promoted", "promotedlink"]) {
                promoted_skipped += 1;
                continue;
            }
            let Some(post) = parse_post(element, page, now) else {
                continue;
            };
            if seen.insert(post.id.clone()) {
                posts.push(post);
            }
        }
        if posts.is_empty() && promoted_skipped == 0 {
            // A listing container with no posts is a legitimate empty feed.
            return Ok(posts);
        }
        Ok(posts)
    }

    fn parse_comments(
        &self,
        doc: &Html,
        page: &Url,
        now: i64,
    ) -> Result<Vec<Comment>, ProviderError> {
        let container = first_in(
            doc.root_element(),
            "div.commentarea div.nestedlisting, div.commentarea div.listing, div.nestedlisting",
        );
        let Some(container) = container else {
            if let Some(gate) = detect_gate(doc) {
                return Err(gate);
            }
            return Err(structure_error(
                "Old Reddit",
                "no comment listing was found",
            ));
        };
        let mut comments = Vec::new();
        let counter = AtomicU64::new(0);
        for comment in direct_children(container, "div", "thing") {
            // `morechildren` stubs need Reddit's JSON API, which an HTML
            // provider does not call. Ignore them rather than showing a fake
            // comment or a button that cannot work.
            if has_class(comment, "comment") {
                comments.push(parse_comment(comment, page, now, &counter));
            }
        }
        Ok(comments)
    }

    fn parse_subreddit(&self, doc: &Html, fallback_name: &str) -> Subreddit {
        let mut subreddit = Subreddit::new(fallback_name);
        if let Some(name) = first_in(doc.root_element(), "span.pagename a")
            .or_else(|| first_in(doc.root_element(), "span.redditname a"))
            .map(text)
        {
            let name = name.trim().trim_start_matches("r/").to_string();
            if !name.is_empty() {
                subreddit.name = name.clone();
                if subreddit.title.is_empty() || subreddit.title == fallback_name {
                    subreddit.title = name;
                }
            }
        }
        if let Some(title) = first_in(doc.root_element(), ".titlebox h1")
            .or_else(|| first_in(doc.root_element(), "title"))
            .map(text)
            .filter(|value| !value.is_empty())
        {
            let title = title.split('•').next().unwrap_or(&title).trim().to_string();
            if !title.is_empty() {
                subreddit.title = title;
            }
        }
        subreddit.description = first_in(doc.root_element(), ".side .usertext-body .md")
            .map(html_to_text)
            .filter(|value| !value.is_empty())
            .or_else(|| meta_content(doc, "description"))
            .unwrap_or_default();
        subreddit.subscribers = first_in(doc.root_element(), ".subscribers .number")
            .map(text)
            .map(|value| parse_count(&value) as u64)
            .unwrap_or(0);
        subreddit.active_users =
            first_in(doc.root_element(), ".users-online .number, .users-online")
                .map(text)
                .map(|value| parse_count(&value) as u64)
                .unwrap_or(0);
        subreddit.over18 = first_in(doc.root_element(), ".over18").is_some()
            || doc
                .select(&selector("body.over18, div.thing.over18"))
                .next()
                .is_some();
        subreddit
    }
}

#[async_trait]
impl ContentProvider for OldRedditProvider {
    fn name(&self) -> &'static str {
        "Old Reddit HTML"
    }

    fn short_name(&self) -> &'static str {
        "OLD REDDIT"
    }

    fn is_demo(&self) -> bool {
        false
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            search: true,
            search_in_subreddit: true,
            saved: false,
            images: true,
            oauth: false,
            demo: false,
        }
    }

    fn supported_sorts(&self, spec: &FeedSpec) -> Vec<FeedSort> {
        sort_options(spec)
    }

    async fn get_feed(&self, request: FeedRequest) -> Result<FeedPage, ProviderError> {
        let url = if let Some(after) = &request.after {
            Url::parse(after).map_err(|_| {
                ProviderError::InvalidInput("invalid pagination URL from the HTML source".into())
            })?
        } else {
            listing_url(self.client.base(), &request, super::HtmlFlavor::OldReddit)?
        };
        let page = self.client.fetch(&url).await?;
        let doc = Html::parse_document(&page.body);
        let now = Utc::now().timestamp();
        let posts = self.parse_feed(&doc, &page.url, now)?;
        if let FeedSpec::Subreddit(name) = &request.spec
            && !posts.is_empty()
        {
            self.state
                .remember_subreddit(self.parse_subreddit(&doc, name));
        }
        self.state.remember_posts(&posts);
        let next = next_page_url(&doc, &page.url);
        Ok(FeedPage {
            posts,
            has_more: next.is_some(),
            after: next,
        })
    }

    async fn get_post(&self, post_id: &str) -> Result<Post, ProviderError> {
        if let Some(post) = self.state.cached_post(post_id) {
            return Ok(post);
        }
        let Some(url) = self.state.post_url(post_id) else {
            return Err(ProviderError::NotFound(format!(
                "post {post_id} is not in the current HTML feed"
            )));
        };
        let url =
            Url::parse(&url).map_err(|e| ProviderError::Parse(format!("invalid post URL: {e}")))?;
        let page = self.client.fetch(&url).await?;
        let doc = Html::parse_document(&page.body);
        let now = Utc::now().timestamp();
        let posts = self.parse_feed(&doc, &page.url, now)?;
        let post = posts
            .into_iter()
            .find(|post| post.id == post_id)
            .ok_or_else(|| {
                ProviderError::NotFound(format!("post {post_id} was not on the HTML page"))
            })?;
        self.state.remember_posts(std::slice::from_ref(&post));
        Ok(post)
    }

    async fn get_comments(
        &self,
        post_id: &str,
        sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        let url = comments_url(
            &self.state,
            self.client.base(),
            post_id,
            sort,
            true,
            super::HtmlFlavor::OldReddit,
        )?;
        let page = self.client.fetch(&url).await?;
        let doc = Html::parse_document(&page.body);
        let now = Utc::now().timestamp();
        self.parse_comments(&doc, &page.url, now)
    }

    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, ProviderError> {
        let name = crate::utils::text::normalize_subreddit(name)
            .ok_or_else(|| ProviderError::InvalidInput("invalid subreddit name".into()))?;
        if let Some(subreddit) = self.state.cached_subreddit(&name) {
            return Ok(subreddit);
        }
        let url = self.client.base().join(&format!("/r/{name}/"))?;
        let page = self.client.fetch(&url).await?;
        let doc = Html::parse_document(&page.body);
        let subreddit = self.parse_subreddit(&doc, &name);
        self.state.remember_subreddit(subreddit.clone());
        Ok(subreddit)
    }
}

fn parse_post(element: ElementRef<'_>, page: &Url, now: i64) -> Option<Post> {
    let id = post_id(element)?;
    let title = first_in(element, "a.title")
        .or_else(|| first_in(element, "p.title a"))
        .map(text)
        .unwrap_or_default();
    let author = attr(element, "data-author")
        .map(str::to_string)
        .or_else(|| first_in(element, "a.author").map(text))
        .unwrap_or_else(|| "[deleted]".into());
    let subreddit = attr(element, "data-subreddit")
        .map(str::to_string)
        .or_else(|| {
            first_in(element, "a.subreddit")
                .map(|node| text(node).trim_start_matches("r/").to_string())
        })
        .unwrap_or_default();
    let score = attr(element, "data-score")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| {
            first_in(element, "div.score, span.score")
                .map(text)
                .map(|value| parse_count(&value))
                .unwrap_or(0)
        });
    let num_comments = attr(element, "data-comments-count")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| {
            first_in(element, "a.comments")
                .map(text)
                .map(|value| parse_count(&value).max(0) as u32)
                .unwrap_or(0)
        });
    let created_utc = attr(element, "data-timestamp")
        .and_then(|value| parse_timestamp(value, now))
        .or_else(|| {
            first_in(element, "time.live-timestamp, time").and_then(|node| {
                attr(node, "datetime")
                    .and_then(|value| parse_timestamp(value, now))
                    .or_else(|| attr(node, "title").and_then(|value| parse_timestamp(value, now)))
                    .or_else(|| Some(parse_timestamp(&text(node), now)).flatten())
            })
        })
        .unwrap_or(0);

    let permalink = attr(element, "data-permalink")
        .and_then(|value| absolute_url(page, value))
        .or_else(|| {
            first_in(element, "a.comments")
                .and_then(|node| attr(node, "href"))
                .and_then(|href| absolute_url(page, href))
        })
        .unwrap_or_default();
    let external = attr(element, "data-url")
        .and_then(|value| absolute_url(page, value))
        .or_else(|| {
            first_in(element, "a.title")
                .and_then(|node| attr(node, "href"))
                .and_then(|href| absolute_url(page, href))
        });
    let domain = attr(element, "data-domain")
        .map(str::to_string)
        .or_else(|| first_in(element, "span.domain a, span.domain").map(text));

    let body_md = first_in(element, "div.expando div.usertext-body div.md")
        .or_else(|| first_in(element, "div.usertext-body div.md"));
    let body = body_md.map(html_to_text).unwrap_or_default();
    let thumbnail = thumbnail(element, page);
    let nsfw = attr(element, "data-nsfw").is_some_and(|value| value.eq_ignore_ascii_case("true"))
        || has_any_class(element, &["over18", "nsfw"]);
    let spoiler = attr(element, "data-spoiler")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        || has_class(element, "spoiler");
    let stickied = attr(element, "data-stickied")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        || has_class(element, "stickied");
    let locked = has_class(element, "locked");
    let removed = has_any_class(element, &["removed", "deleted"])
        || title.eq_ignore_ascii_case("[removed]")
        || title.eq_ignore_ascii_case("[deleted]");

    let content = if removed {
        PostContent::Removed
    } else {
        infer_content(
            element,
            page,
            external.as_deref(),
            thumbnail.as_deref(),
            &body,
        )
    };
    let is_self = matches!(content, PostContent::Text);
    let flair = first_in(element, "span.flair, a.flair")
        .map(text)
        .filter(|value| !value.is_empty());

    Some(Post {
        id,
        subreddit,
        author,
        title,
        body,
        created_utc,
        score,
        num_comments,
        permalink,
        content,
        flair,
        nsfw,
        spoiler,
        stickied,
        locked,
        removed,
        is_self,
        domain,
        thumbnail,
    })
}

fn infer_content(
    element: ElementRef<'_>,
    page: &Url,
    external: Option<&str>,
    thumbnail: Option<&str>,
    body: &str,
) -> PostContent {
    let gallery = gallery_items(element, page);
    let gallery_marked =
        has_class(element, "gallery") || attr(element, "data-gallery-items").is_some();
    if gallery_marked {
        if gallery.len() > 1 {
            return PostContent::Gallery { items: gallery };
        }
        if let Some(url) = gallery.into_iter().next() {
            return PostContent::Image { url: url.url };
        }
        if let Some(url) = external {
            return PostContent::Link {
                url: url.to_string(),
                thumbnail: thumbnail.map(str::to_string),
            };
        }
    }
    if has_class(element, "self") {
        return PostContent::Text;
    }
    if let Some(url) = external {
        if is_image_url(url) {
            return PostContent::Image {
                url: url.to_string(),
            };
        }
        if is_video_url(url) {
            return PostContent::Video {
                url: url.to_string(),
                thumbnail: thumbnail.map(str::to_string),
                duration: None,
            };
        }
    }
    if !body.trim().is_empty() {
        return PostContent::Text;
    }
    if let Some(url) = external {
        if is_gallery_url(url) {
            return PostContent::Link {
                url: url.to_string(),
                thumbnail: thumbnail.map(str::to_string),
            };
        }
        return PostContent::Link {
            url: url.to_string(),
            thumbnail: thumbnail.map(str::to_string),
        };
    }
    PostContent::Text
}

fn gallery_items(element: ElementRef<'_>, page: &Url) -> Vec<MediaItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    if let Some(raw) = attr(element, "data-gallery-items")
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(raw)
        && let Some(array) = value.as_array()
    {
        for item in array {
            let (raw_url, width, height) = match item {
                serde_json::Value::String(url) => (url.as_str(), None, None),
                serde_json::Value::Object(map) => (
                    map.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    map.get("width").and_then(|v| v.as_u64()).map(|v| v as u32),
                    map.get("height").and_then(|v| v.as_u64()).map(|v| v as u32),
                ),
                _ => continue,
            };
            let Some(url) = absolute_url(page, raw_url) else {
                continue;
            };
            if seen.insert(url.clone()) {
                items.push(match (width, height) {
                    (Some(width), Some(height)) => MediaItem::with_size(url, width, height),
                    _ => MediaItem::new(url),
                });
            }
        }
    }
    if items.is_empty() {
        collect_preview_images(element, page, &mut items, &mut seen);
    }
    items
}

fn collect_preview_images(
    element: ElementRef<'_>,
    page: &Url,
    items: &mut Vec<MediaItem>,
    seen: &mut HashSet<String>,
) {
    for image in all_in(
        element,
        "div.gallery-preview img, div.media-preview img, a.thumbnail img",
    ) {
        let Some(raw) = attr(image, "src") else {
            continue;
        };
        let Some(url) = absolute_url(page, raw) else {
            continue;
        };
        if seen.insert(url.clone()) {
            items.push(MediaItem::new(url));
        }
    }
}

fn thumbnail(element: ElementRef<'_>, page: &Url) -> Option<String> {
    let raw = first_in(element, "img.thumbnail")
        .and_then(|node| attr(node, "src"))
        .or_else(|| attr(element, "data-thumbnail"))?;
    if matches!(
        raw.to_ascii_lowercase().as_str(),
        "self" | "default" | "nsfw" | "spoiler" | "image" | "link"
    ) {
        return None;
    }
    absolute_url(page, raw).filter(|url| is_image_url(url))
}

fn post_id(element: ElementRef<'_>) -> Option<String> {
    if let Some(id) = attr(element, "data-fullname").and_then(|value| value.strip_prefix("t3_")) {
        return Some(id.to_string());
    }
    if let Some(id) = attr(element, "id").and_then(|value| value.strip_prefix("thing_t3_")) {
        return Some(id.to_string());
    }
    first_in(element, "a.comments")
        .and_then(|node| attr(node, "href"))
        .and_then(|href| Url::parse(href).ok())
        .and_then(|url| {
            let mut parts = url.path_segments()?;
            parts.find(|part| *part == "comments")?;
            parts.next().map(str::to_string)
        })
}

fn parse_comment(element: ElementRef<'_>, page: &Url, now: i64, counter: &AtomicU64) -> Comment {
    let id = comment_id(element, counter);
    let entry = first_in(element, "div.entry").unwrap_or(element);
    let author = first_in(entry, "a.author")
        .map(text)
        .or_else(|| attr(element, "data-author").map(str::to_string))
        .unwrap_or_else(|| "[deleted]".into());
    let score = attr(element, "data-score")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| {
            first_in(entry, "span.score, p.tagline span.score")
                .map(text)
                .map(|value| parse_count(&value))
                .unwrap_or(0)
        });
    let created_utc = first_in(entry, "time.live-timestamp, time")
        .and_then(|node| {
            attr(node, "datetime")
                .and_then(|value| parse_timestamp(value, now))
                .or_else(|| attr(node, "title").and_then(|value| parse_timestamp(value, now)))
                .or_else(|| parse_timestamp(&text(node), now))
        })
        .unwrap_or(0);
    let body = first_in(entry, "div.usertext-body div.md")
        .map(|node| html_to_text(node))
        .unwrap_or_default();
    let deleted = has_any_class(element, &["deleted", "removed"])
        || author == "[deleted]"
        || body == "[deleted]";
    let removed = has_class(element, "removed") || body == "[removed]";
    let mut comment = Comment::new(
        id,
        author,
        if body.is_empty() && deleted {
            "[deleted]".into()
        } else {
            body
        },
    );
    comment.score = score;
    comment.created_utc = created_utc;
    comment.deleted = deleted;
    comment.removed = removed;

    if let Some(child) = direct_child(element, "div", "child") {
        let listing = first_in(child, "div.sitetable, div.listing").unwrap_or(child);
        for reply in direct_children(listing, "div", "thing") {
            if has_class(reply, "comment") {
                let mut parsed = parse_comment(reply, page, now, counter);
                if parsed.parent_id.is_none() {
                    parsed.parent_id = Some(comment.id.clone());
                }
                comment.replies.push(parsed);
            }
        }
    }
    let _ = page;
    comment
}

fn comment_id(element: ElementRef<'_>, counter: &AtomicU64) -> String {
    if let Some(id) = attr(element, "data-fullname").and_then(|value| value.strip_prefix("t1_")) {
        return id.to_string();
    }
    if let Some(id) = attr(element, "id") {
        if let Some(id) = id.strip_prefix("thing_t1_") {
            return id.to_string();
        }
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if let Some(id) = first_in(element, "a.bylink")
        .and_then(|node| attr(node, "href"))
        .and_then(|href| Url::parse(href).ok())
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut parts| parts.next_back().map(str::to_string))
        })
    {
        return id;
    }
    format!(
        "oldreddit_comment_{}",
        counter.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = include_str!("../../../fixtures/html/old_reddit_feed.html");
    const COMMENTS: &str = include_str!("../../../fixtures/html/old_reddit_comments.html");
    const LOGIN: &str = include_str!("../../../fixtures/html/login_wall.html");
    const BROKEN: &str = include_str!("../../../fixtures/html/structure_changed.html");

    fn page() -> Url {
        Url::parse("https://old.reddit.com/r/rust/").unwrap()
    }

    #[test]
    fn parses_feed_posts_content_and_pagination() {
        let doc = Html::parse_document(FEED);
        let provider = OldRedditProvider::new(HtmlClientConfig::new(
            "https://old.reddit.com",
            "reddtty/test",
        ))
        .unwrap();
        let posts = provider.parse_feed(&doc, &page(), 1_700_000_000).unwrap();
        assert_eq!(posts.len(), 6);
        assert_eq!(posts[0].id, "aaa");
        assert_eq!(posts[0].author, "alice");
        assert_eq!(posts[0].subreddit, "rust");
        assert_eq!(posts[0].score, 1234);
        assert_eq!(posts[0].num_comments, 56);
        assert!(matches!(posts[0].content, PostContent::Text));
        assert!(posts[0].body.contains("Body paragraph"));
        assert!(matches!(posts[1].content, PostContent::Image { .. }));
        match &posts[2].content {
            PostContent::Gallery { items } => {
                assert_eq!(items.len(), 2);
                assert!(items[0].url.ends_with("gallery-one.jpg"));
            }
            other => panic!("expected gallery, got {other:?}"),
        }
        assert!(matches!(posts[3].content, PostContent::Link { .. }));
        assert!(matches!(posts[4].content, PostContent::Video { .. }));
        assert!(posts[5].nsfw);
        assert!(posts[5].spoiler);
        assert_eq!(
            next_page_url(&doc, &page()).as_deref(),
            Some("https://old.reddit.com/r/rust/?count=25&after=t3_next")
        );
    }

    #[test]
    fn parses_nested_and_deleted_comments_without_wrong_parents() {
        let doc = Html::parse_document(COMMENTS);
        let provider = OldRedditProvider::new(HtmlClientConfig::new(
            "https://old.reddit.com",
            "reddtty/test",
        ))
        .unwrap();
        let comments = provider
            .parse_comments(&doc, &page(), 1_700_000_000)
            .unwrap();
        // The `morechildren` stub is ignored because HTML mode has no
        // documented load-more mechanism.
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].id, "c1");
        assert_eq!(comments[0].author, "bob");
        assert_eq!(comments[0].score, 12);
        assert_eq!(comments[0].replies.len(), 1);
        assert_eq!(comments[0].replies[0].id, "c2");
        assert_eq!(comments[0].replies[0].parent_id.as_deref(), Some("c1"));
        assert_eq!(comments[0].replies[0].replies.len(), 1);
        assert_eq!(comments[0].replies[0].replies[0].id, "c3");
        assert!(comments[1].deleted);
    }

    #[test]
    fn login_and_structure_pages_are_not_empty_feeds() {
        let login = Html::parse_document(LOGIN);
        let provider = OldRedditProvider::new(HtmlClientConfig::new(
            "https://old.reddit.com",
            "reddtty/test",
        ))
        .unwrap();
        assert!(matches!(
            provider.parse_feed(&login, &page(), 1_700_000_000),
            Err(ProviderError::LoginRequired(_))
        ));
        let broken = Html::parse_document(BROKEN);
        assert!(matches!(
            provider.parse_feed(&broken, &page(), 1_700_000_000),
            Err(ProviderError::HtmlStructureChanged(_))
        ));
    }

    #[test]
    fn missing_fields_do_not_panic() {
        let doc = Html::parse_document(
            r#"<div id="siteTable"><div class="thing link" data-fullname="t3_x"></div></div>"#,
        );
        let provider = OldRedditProvider::new(HtmlClientConfig::new(
            "https://old.reddit.com",
            "reddtty/test",
        ))
        .unwrap();
        let posts = provider.parse_feed(&doc, &page(), 1_700_000_000).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "");
        assert_eq!(posts[0].author, "[deleted]");
        assert_eq!(posts[0].created_utc, 0);
    }
}
