//! Redlib HTML provider.
//!
//! Redlib is an independent third-party front end. Its HTML is not Reddit's
//! API and is not the same as Old Reddit's markup. This module has its own
//! parser and never pretends Redlib is an official Reddit API.

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

pub struct RedlibProvider {
    client: HtmlClient,
    state: HtmlState,
}

impl RedlibProvider {
    pub fn new(config: HtmlClientConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            client: HtmlClient::new(&config)?,
            state: HtmlState::default(),
        })
    }

    fn parse_feed(&self, doc: &Html, page: &Url, now: i64) -> Result<Vec<Post>, ProviderError> {
        let main = first_in(doc.root_element(), "main, #main, div.posts");
        let Some(main) = main else {
            if let Some(gate) = detect_gate(doc) {
                return Err(gate);
            }
            return Err(structure_error(
                "Redlib",
                "no <main> or post container was found",
            ));
        };
        let mut posts = Vec::new();
        let mut seen = HashSet::new();
        for element in doc.select(&selector("div.post")) {
            if has_class(element, "highlighted") && doc.select(&selector("div.post")).count() > 1 {
                // Detail pages wrap the post in `.post.highlighted`; feed cards
                // are not marked highlighted.
                continue;
            }
            let Some(post) = parse_post(element, page, now) else {
                continue;
            };
            if seen.insert(post.id.clone()) {
                posts.push(post);
            }
        }
        let _ = main;
        Ok(posts)
    }

    fn parse_comments(
        &self,
        doc: &Html,
        page: &Url,
        now: i64,
    ) -> Result<Vec<Comment>, ProviderError> {
        let main = first_in(doc.root_element(), "main, #main");
        let Some(main) = main else {
            if let Some(gate) = detect_gate(doc) {
                return Err(gate);
            }
            return Err(structure_error("Redlib", "no <main> element was found"));
        };
        let container = first_in(main, "div.comments").unwrap_or(main);
        let mut roots = direct_children(container, "div", "thread");
        if roots.is_empty() {
            roots = all_in(container, "div.thread")
                .into_iter()
                .filter(|thread| {
                    !thread
                        .ancestors()
                        .filter_map(ElementRef::wrap)
                        .any(|ancestor| has_class(ancestor, "thread"))
                })
                .collect();
        }
        let counter = AtomicU64::new(0);
        Ok(roots
            .into_iter()
            .filter_map(|thread| parse_thread(thread, page, now, &counter))
            .collect())
    }

    fn parse_subreddit(&self, doc: &Html, fallback_name: &str) -> Subreddit {
        let mut subreddit = Subreddit::new(fallback_name);
        if let Some(name) = first_in(doc.root_element(), "a.post_subreddit")
            .map(|node| text(node).trim_start_matches("r/").to_string())
            .filter(|value| !value.is_empty())
        {
            subreddit.name = name;
        }
        if let Some(title) = first_in(doc.root_element(), "title")
            .map(text)
            .filter(|value| !value.is_empty())
        {
            subreddit.title = title
                .split("- r/")
                .next()
                .unwrap_or(&title)
                .trim()
                .to_string();
        }
        subreddit.description = first_in(doc.root_element(), "p#sub_description")
            .map(text)
            .filter(|value| !value.is_empty())
            .or_else(|| meta_content(doc, "description"))
            .unwrap_or_default();
        subreddit.subscribers = first_in(doc.root_element(), ".subscribers, #sub_subscribers")
            .map(text)
            .map(|value| parse_count(&value) as u64)
            .unwrap_or(0);
        subreddit.active_users = first_in(doc.root_element(), ".active_users, .users_online")
            .map(text)
            .map(|value| parse_count(&value) as u64)
            .unwrap_or(0);
        subreddit.over18 = first_in(doc.root_element(), ".over18, .nsfw").is_some()
            || has_class(doc.root_element(), "over18");
        subreddit
    }
}

#[async_trait]
impl ContentProvider for RedlibProvider {
    fn name(&self) -> &'static str {
        "Redlib HTML"
    }

    fn short_name(&self) -> &'static str {
        "REDLIB"
    }

    fn display_name(&self) -> String {
        format!(
            "REDLIB · {}",
            self.client.base().host_str().unwrap_or("unknown")
        )
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
                ProviderError::InvalidInput("invalid pagination URL from Redlib".into())
            })?
        } else {
            listing_url(self.client.base(), &request, super::HtmlFlavor::Redlib)?
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
                "post {post_id} is not in the current Redlib feed"
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
                ProviderError::NotFound(format!("post {post_id} was not on the Redlib page"))
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
            false,
            super::HtmlFlavor::Redlib,
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
    // Redlib puts flair links inside the same <h2> before the real post link.
    // Choose the anchor that points at /comments/, not the first anchor.
    let title_link = all_in(element, "h2.post_title a")
        .into_iter()
        .find(|node| attr(*node, "href").is_some_and(|href| href.contains("/comments/")))
        .or_else(|| first_in(element, "a.post_title"))
        .or_else(|| first_in(element, "h2.post_title a"))
        .or_else(|| first_in(element, "h2 a"));
    let title = title_link.map(text).unwrap_or_default();
    let id = post_id(element, page, title_link)?;
    let author = first_in(element, "a.post_author")
        .map(text)
        .map(|value| value.trim_start_matches("u/").to_string())
        .or_else(|| attr(element, "data-author").map(str::to_string))
        .unwrap_or_else(|| "[deleted]".into());
    let subreddit = first_in(element, "a.post_subreddit")
        .map(text)
        .map(|value| value.trim_start_matches("r/").to_string())
        .or_else(|| {
            title_link
                .and_then(|node| attr(node, "href"))
                .and_then(|href| Url::parse(href).ok())
                .and_then(|url| {
                    let mut parts = url.path_segments()?;
                    (parts.next()? == "r").then(|| parts.next().map(str::to_string))?
                })
        })
        .unwrap_or_default();
    let score = first_in(element, "div.post_score, span.post_score")
        .and_then(|node| {
            attr(node, "title")
                .map(str::to_string)
                .or_else(|| Some(text(node)))
        })
        .map(|value| parse_count(&value))
        .unwrap_or_else(|| attr(element, "data-score").map(parse_count).unwrap_or(0));
    let num_comments = first_in(element, "a.post_comments")
        .and_then(|node| {
            attr(node, "title")
                .map(str::to_string)
                .or_else(|| Some(text(node)))
        })
        .map(|value| parse_count(&value).max(0) as u32)
        .unwrap_or_else(|| {
            attr(element, "data-comments-count")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        });
    let created_utc = first_in(element, "span.created, a.created")
        .and_then(|node| {
            attr(node, "title")
                .and_then(|value| parse_timestamp(value, now))
                .or_else(|| attr(node, "datetime").and_then(|value| parse_timestamp(value, now)))
                .or_else(|| parse_timestamp(&text(node), now))
        })
        .unwrap_or_else(|| {
            attr(element, "data-timestamp")
                .and_then(|value| parse_timestamp(value, now))
                .unwrap_or(0)
        });
    let comments_href = first_in(element, "a.post_comments")
        .and_then(|node| attr(node, "href"))
        .or_else(|| title_link.and_then(|node| attr(node, "href")));
    let permalink = comments_href
        .and_then(|href| absolute_url(page, href))
        .unwrap_or_default();
    let external = first_in(element, "a#post_url, a.post_url")
        .and_then(|node| attr(node, "href"))
        .and_then(|href| absolute_url(page, href));
    let body = first_in(element, "div.post_body div.md, div.post_body, div.md")
        .map(html_to_text)
        .unwrap_or_default();
    let thumbnail = first_in(
        element,
        "img.post_thumbnail, a.post_thumbnail img, img.thumbnail, a.post_media_image img, div.post_media_content img",
    )
    .and_then(|node| attr(node, "src"))
    .or_else(|| {
        first_in(element, "video.post_media_video")
            .and_then(|node| attr(node, "poster"))
    })
    .and_then(|src| absolute_url(page, src))
    .filter(|url| is_image_url(url));
    let nsfw = has_any_class(element, &["nsfw", "over18"])
        || first_in(element, "span.nsfw, .post_nsfw").is_some();
    let spoiler = has_class(element, "spoiler");
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
    let domain = external
        .as_deref()
        .and_then(|url| Url::parse(url).ok())
        .and_then(|url| url.host_str().map(str::to_string));
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
        flair: first_in(element, "span.flair, a.flair")
            .map(text)
            .filter(|value| !value.is_empty()),
        nsfw,
        spoiler,
        stickied: has_class(element, "stickied") || has_class(element, "pinned"),
        locked: has_class(element, "locked"),
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
    if let Some(video) = first_in(element, "video.post_media_video")
        && let Some(src) = attr(video, "src").and_then(|src| absolute_url(page, src))
    {
        let poster = attr(video, "poster")
            .and_then(|poster| absolute_url(page, poster))
            .filter(|url| is_image_url(url));
        return PostContent::Video {
            url: src,
            thumbnail: poster,
            duration: None,
        };
    }
    let gallery = gallery_items(element, page);
    if gallery.len() > 1 {
        return PostContent::Gallery { items: gallery };
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
    if let Some(item) = gallery.into_iter().next()
        && is_image_url(&item.url)
    {
        return PostContent::Image { url: item.url };
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
    for image in all_in(
        element,
        "div.gallery img, div.media-gallery img, a.post_thumbnail img, a.post_media_image img, div.post_media_content img, img.post_image",
    ) {
        let Some(src) = attr(image, "src") else {
            continue;
        };
        let Some(url) = absolute_url(page, src) else {
            continue;
        };
        if seen.insert(url.clone()) {
            items.push(MediaItem::new(url));
        }
    }
    items
}

fn post_id(
    element: ElementRef<'_>,
    page: &Url,
    title_link: Option<ElementRef<'_>>,
) -> Option<String> {
    if let Some(id) = attr(element, "id") {
        if let Some(id) = id.strip_prefix("post_") {
            return Some(id.to_string());
        }
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    if let Some(id) = attr(element, "data-fullname").and_then(|value| value.strip_prefix("t3_")) {
        return Some(id.to_string());
    }
    let href = title_link
        .and_then(|node| attr(node, "href"))
        .or_else(|| first_in(element, "a.post_comments").and_then(|node| attr(node, "href")))?;
    let absolute = absolute_url(page, href)?;
    let url = Url::parse(&absolute).ok()?;
    let mut parts = url.path_segments()?;
    let mut seen_comments = false;
    for part in parts.by_ref() {
        if part == "comments" {
            seen_comments = true;
            break;
        }
    }
    seen_comments.then(|| parts.next().map(str::to_string))?
}

fn parse_thread(
    thread: ElementRef<'_>,
    page: &Url,
    now: i64,
    counter: &AtomicU64,
) -> Option<Comment> {
    let comment = direct_child(thread, "div", "comment")?;
    Some(parse_comment_node(comment, page, now, counter))
}

fn parse_comment_node(
    comment: ElementRef<'_>,
    page: &Url,
    now: i64,
    counter: &AtomicU64,
) -> Comment {
    let id = attr(comment, "id")
        .map(|id| id.trim_start_matches("comment_").to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| format!("redlib_comment_{}", counter.fetch_add(1, Ordering::Relaxed)));
    let author = first_in(comment, "a.comment_author")
        .map(text)
        .map(|value| value.trim_start_matches("u/").to_string())
        .unwrap_or_else(|| "[deleted]".into());
    let score = first_in(
        comment,
        "p.comment_score, div.comment_score, span.comment_score",
    )
    .map(|node| {
        if attr(node, "title").is_some_and(|title| title.eq_ignore_ascii_case("hidden")) {
            0
        } else {
            attr(node, "title")
                .map(parse_count)
                .unwrap_or_else(|| parse_count(&text(node)))
        }
    })
    .unwrap_or(0);
    let created_utc = first_in(comment, "a.created, span.created")
        .and_then(|node| {
            attr(node, "title")
                .and_then(|value| parse_timestamp(value, now))
                .or_else(|| parse_timestamp(&text(node), now))
        })
        .unwrap_or(0);
    let body = first_in(
        comment,
        "div.comment_body div.md, details.comment_right div.md, div.md",
    )
    .map(html_to_text)
    .unwrap_or_default();
    let deleted = has_any_class(comment, &["deleted", "removed"])
        || author == "[deleted]"
        || body == "[deleted]";
    let removed = has_class(comment, "removed") || body == "[removed]";
    let mut node = Comment::new(
        id,
        author,
        if body.is_empty() && deleted {
            "[deleted]".into()
        } else {
            body
        },
    );
    node.score = score;
    node.created_utc = created_utc;
    node.deleted = deleted;
    node.removed = removed;

    if let Some(replies) = first_in(comment, "blockquote.replies, div.replies") {
        // Fixture/older Redlib inserts a `div.thread` wrapper; the live
        // instance puts the reply `div.comment` directly inside the blockquote.
        // Support both without inventing a parent.
        for reply in direct_children(replies, "div", "thread") {
            let Some(mut parsed) = parse_thread(reply, page, now, counter) else {
                continue;
            };
            if parsed.parent_id.is_none() {
                parsed.parent_id = Some(node.id.clone());
            }
            node.replies.push(parsed);
        }
        for reply in direct_children(replies, "div", "comment") {
            let mut parsed = parse_comment_node(reply, page, now, counter);
            if parsed.parent_id.is_none() {
                parsed.parent_id = Some(node.id.clone());
            }
            node.replies.push(parsed);
        }
    }
    let _ = page;
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = include_str!("../../../fixtures/html/redlib_feed.html");
    const COMMENTS: &str = include_str!("../../../fixtures/html/redlib_comments.html");
    const LOGIN: &str = include_str!("../../../fixtures/html/login_wall.html");

    fn page() -> Url {
        Url::parse("https://redlib.example/r/rust/").unwrap()
    }

    #[test]
    fn parses_redlib_feed_and_pagination() {
        let doc = Html::parse_document(FEED);
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.example",
            "reddtty/test",
        ))
        .unwrap();
        let posts = provider.parse_feed(&doc, &page(), 1_700_000_000).unwrap();
        assert_eq!(posts.len(), 6);
        assert_eq!(posts[0].id, "aaa");
        assert_eq!(posts[0].title, "Text post title");
        assert_eq!(posts[0].author, "alice");
        assert_eq!(posts[0].subreddit, "rust");
        assert_eq!(posts[0].score, 1234);
        assert_eq!(posts[0].num_comments, 56);
        assert!(matches!(posts[0].content, PostContent::Text));
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
        assert!(posts[5].removed);
        assert_eq!(
            next_page_url(&doc, &page()).as_deref(),
            Some("https://redlib.example/r/rust/?sort=hot&t=&after=t3_next")
        );
    }

    #[test]
    fn parses_live_media_markup_from_redlib() {
        let html = r#"
        <main><div class="posts">
          <div class="post" id="post_img1">
            <a class="post_subreddit" href="/r/pics">r/pics</a>
            <a class="post_author" href="/u/alice">u/alice</a>
            <span class="created" title="2024-01-01T00:00:00Z">1h ago</span>
            <h2 class="post_title"><a href="/r/pics/comments/img1/photo/">A photo</a></h2>
            <div class="post_score" title="5">5</div>
            <a class="post_comments" title="2" href="/r/pics/comments/img1/photo/">2 comments</a>
            <div class="post_media_content">
              <a href="/img/photo.jpeg" class="post_media_image short">
                <img width="100%" height="100%" alt="Post image" src="/img/photo.jpeg"/>
              </a>
            </div>
          </div>
          <div class="post" id="post_vid1">
            <a class="post_subreddit" href="/r/videos">r/videos</a>
            <a class="post_author" href="/u/bob">u/bob</a>
            <span class="created" title="2024-01-02T00:00:00Z">2h ago</span>
            <h2 class="post_title"><a href="/r/videos/comments/vid1/clip/">A clip</a></h2>
            <div class="post_score" title="7">7</div>
            <a class="post_comments" title="1" href="/r/videos/comments/vid1/clip/">1 comment</a>
            <div class="post_media_content">
              <video class="post_media_video short" src="/packaged/vid1/muxed-high.mp4?m=DASHPlaylist.mpd"
                     poster="/preview/external-pre/poster.png?format=pjpg" preload="none" controls></video>
            </div>
          </div>
        </div></main>
        "#;
        let doc = Html::parse_document(html);
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.example",
            "reddtty/test",
        ))
        .unwrap();
        let posts = provider.parse_feed(&doc, &page(), 1_700_000_000).unwrap();
        assert_eq!(posts.len(), 2);
        match &posts[0].content {
            PostContent::Image { url } => {
                assert!(url.ends_with("/img/photo.jpeg"), "{url}");
            }
            other => panic!("expected image, got {other:?}"),
        }
        assert!(
            posts[0]
                .thumbnail
                .as_deref()
                .is_some_and(|url| url.ends_with("/img/photo.jpeg"))
        );
        match &posts[1].content {
            PostContent::Video { url, thumbnail, .. } => {
                assert!(
                    url.ends_with("/packaged/vid1/muxed-high.mp4?m=DASHPlaylist.mpd"),
                    "{url}"
                );
                assert!(
                    thumbnail
                        .as_deref()
                        .is_some_and(|url| url.contains("/preview/external-pre/poster.png")),
                    "{thumbnail:?}"
                );
            }
            other => panic!("expected video, got {other:?}"),
        }
    }

    #[test]
    fn parses_redlib_nested_and_deleted_comments() {
        let doc = Html::parse_document(COMMENTS);
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.example",
            "reddtty/test",
        ))
        .unwrap();
        let comments = provider
            .parse_comments(&doc, &page(), 1_700_000_000)
            .unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].id, "c1");
        assert_eq!(comments[0].replies.len(), 2);
        assert_eq!(comments[0].replies[0].id, "c2");
        assert_eq!(comments[0].replies[0].parent_id.as_deref(), Some("c1"));
        assert_eq!(comments[0].replies[1].id, "c4");
        assert_eq!(comments[0].replies[1].parent_id.as_deref(), Some("c1"));
        assert!(comments[1].deleted);
    }

    #[test]
    fn display_name_includes_the_instance_host() {
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.hbubli.cc",
            "reddtty/test",
        ))
        .unwrap();
        assert_eq!(provider.display_name(), "REDLIB · redlib.hbubli.cc");
    }

    #[test]
    fn redlib_login_page_is_not_an_empty_feed() {
        let doc = Html::parse_document(LOGIN);
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.example",
            "reddtty/test",
        ))
        .unwrap();
        assert!(matches!(
            provider.parse_feed(&doc, &page(), 1_700_000_000),
            Err(ProviderError::LoginRequired(_)) | Err(ProviderError::Forbidden(_))
        ));
    }

    #[test]
    fn redlib_missing_fields_do_not_panic() {
        let doc = Html::parse_document(
            r#"<main><div class="posts"><div class="post" id="post_x"></div></div></main>"#,
        );
        let provider = RedlibProvider::new(HtmlClientConfig::new(
            "https://redlib.example",
            "reddtty/test",
        ))
        .unwrap();
        let posts = provider.parse_feed(&doc, &page(), 1_700_000_000).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "");
        assert_eq!(posts[0].author, "[deleted]");
    }
}
