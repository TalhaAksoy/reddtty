//! Offline demo provider backed by embedded fixture JSON and images.
//!
//! The demo data is clearly synthetic and is never presented as live Reddit
//! content. Timestamps are shifted at startup so relative times stay readable.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use crate::error::ProviderError;
use crate::models::{
    Comment, CommentSort, FeedPage, FeedRequest, FeedSort, FeedSpec, Post, SearchScope, Subreddit,
    TopRange,
};
use crate::providers::traits::{ContentProvider, ProviderCapabilities};
use crate::utils::text::{normalize_subreddit, strip_markdown};

const POSTS_JSON: &str = include_str!("../../fixtures/posts.json");
const COMMENTS_JSON: &str = include_str!("../../fixtures/comments.json");
const SUBREDDITS_JSON: &str = include_str!("../../fixtures/subreddits.json");

const IMAGES: &[(&str, &[u8])] = &[
    (
        "aurora.png",
        include_bytes!("../../fixtures/images/aurora.png"),
    ),
    (
        "mountains.png",
        include_bytes!("../../fixtures/images/mountains.png"),
    ),
    (
        "terminal.png",
        include_bytes!("../../fixtures/images/terminal.png"),
    ),
    ("city.png", include_bytes!("../../fixtures/images/city.png")),
    (
        "waves.png",
        include_bytes!("../../fixtures/images/waves.png"),
    ),
    (
        "desktop.png",
        include_bytes!("../../fixtures/images/desktop.png"),
    ),
    (
        "coffee.png",
        include_bytes!("../../fixtures/images/coffee.png"),
    ),
    ("tall.png", include_bytes!("../../fixtures/images/tall.png")),
];

pub struct DemoProvider {
    posts: Vec<Post>,
    comments: HashMap<String, Vec<Comment>>,
    subreddits: HashMap<String, Subreddit>,
    image_dir: Option<PathBuf>,
    now: i64,
}

impl DemoProvider {
    pub fn new(cache_dir: &Path) -> Self {
        let mut posts: Vec<Post> =
            serde_json::from_str(POSTS_JSON).expect("embedded demo posts.json must be valid");
        let mut comments: HashMap<String, Vec<Comment>> =
            serde_json::from_str(COMMENTS_JSON).expect("embedded demo comments.json must be valid");
        let subreddit_list: Vec<Subreddit> = serde_json::from_str(SUBREDDITS_JSON)
            .expect("embedded demo subreddits.json must be valid");

        let now = Utc::now().timestamp();
        let newest = posts.iter().map(|p| p.created_utc).max().unwrap_or(now);
        let shift = now - (newest + 600);

        for post in &mut posts {
            post.created_utc += shift;
            // Reddit's permalink host, for copy/open actions.
            if !post.permalink.starts_with('/') && !post.permalink.starts_with("http") {
                post.permalink.insert(0, '/');
            }
        }
        for tree in comments.values_mut() {
            for root in tree {
                shift_comment_times(root, shift, now);
            }
        }

        let subreddits = subreddit_list
            .into_iter()
            .map(|s| (s.name.to_ascii_lowercase(), s))
            .collect();

        let image_dir = extract_images(cache_dir).ok();

        Self {
            posts,
            comments,
            subreddits,
            image_dir,
            now,
        }
    }

    pub fn post_count(&self) -> usize {
        self.posts.len()
    }

    fn age_hours(&self, post: &Post) -> f64 {
        ((self.now - post.created_utc).max(0) as f64 / 3600.0).max(0.02)
    }

    fn sort_posts(
        &self,
        posts: &mut Vec<Post>,
        sort: FeedSort,
        top: TopRange,
        query: Option<&str>,
    ) {
        match sort {
            FeedSort::Best => {
                if let Some(query) = query {
                    let tokens = search_tokens(query);
                    posts.sort_by(|a, b| {
                        relevance(b, &tokens)
                            .cmp(&relevance(a, &tokens))
                            .then_with(|| b.score.cmp(&a.score))
                    });
                } else {
                    posts.sort_by(|a, b| {
                        b.score
                            .cmp(&a.score)
                            .then_with(|| b.created_utc.cmp(&a.created_utc))
                    });
                }
            }
            FeedSort::Hot => posts.sort_by(|a, b| {
                hot_score(self, b)
                    .total_cmp(&hot_score(self, a))
                    .then_with(|| b.created_utc.cmp(&a.created_utc))
            }),
            FeedSort::New => posts.sort_by_key(|p| std::cmp::Reverse(p.created_utc)),
            FeedSort::Top => {
                let cutoff = self.now.saturating_sub(top.seconds());
                posts.retain(|p| top == TopRange::All || p.created_utc >= cutoff);
                posts.sort_by(|a, b| {
                    b.score
                        .cmp(&a.score)
                        .then_with(|| b.created_utc.cmp(&a.created_utc))
                });
            }
            FeedSort::Rising => {
                let cutoff = self.now - 172_800;
                posts.retain(|p| p.created_utc >= cutoff);
                posts.sort_by(|a, b| {
                    rising_score(self, b)
                        .total_cmp(&rising_score(self, a))
                        .then_with(|| b.created_utc.cmp(&a.created_utc))
                });
            }
            FeedSort::Controversial => {
                posts.retain(|p| p.score < 900);
                posts.sort_by(|a, b| {
                    b.num_comments
                        .cmp(&a.num_comments)
                        .then_with(|| a.score.cmp(&b.score))
                });
            }
        }
    }

    fn make_more_comments(&self, post_id: &str, children: &[String]) -> Vec<Comment> {
        let mut out = Vec::new();
        for (index, child) in children.iter().enumerate() {
            let seed = hash(format!("{post_id}:{child}"));
            let author = DEMO_REPLY_AUTHORS[(seed as usize + index) % DEMO_REPLY_AUTHORS.len()];
            let body = DEMO_REPLY_BODIES[(seed as usize / 7 + index) % DEMO_REPLY_BODIES.len()];
            let mut comment = Comment::new(child.clone(), author, body);
            comment.score = (seed % 180) as i64 - 10;
            comment.created_utc = self.now - 600 - (seed % 5_400) as i64;
            comment.replies.push({
                let mut reply = Comment::new(
                    format!("{child}_r"),
                    DEMO_REPLY_AUTHORS[(seed as usize + 3) % DEMO_REPLY_AUTHORS.len()],
                    DEMO_REPLY_BODIES[(seed as usize / 11 + 1) % DEMO_REPLY_BODIES.len()],
                );
                reply.parent_id = Some(child.clone());
                reply.score = (seed % 70) as i64;
                reply.created_utc = self.now - 300;
                reply
            });
            out.push(comment);
        }
        out
    }
}

fn extract_images(cache_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = cache_dir.join("demo-images");
    fs::create_dir_all(&dir)?;
    for (name, bytes) in IMAGES {
        let path = dir.join(name);
        let needs_write = match fs::metadata(&path) {
            Ok(meta) => meta.len() != bytes.len() as u64,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&path, bytes)?;
        }
    }
    Ok(dir)
}

fn shift_comment_times(comment: &mut Comment, shift: i64, now: i64) {
    comment.created_utc = (comment.created_utc + shift).min(now - 60);
    for reply in &mut comment.replies {
        shift_comment_times(reply, shift, now);
    }
}

fn hot_score(provider: &DemoProvider, post: &Post) -> f64 {
    let age = provider.age_hours(post);
    let engagement = post.score.max(0) as f64 + post.num_comments as f64 * 3.0;
    engagement / (age + 2.0).powf(1.35)
}

fn rising_score(provider: &DemoProvider, post: &Post) -> f64 {
    let age = provider.age_hours(post);
    (post.score.max(0) as f64 + post.num_comments as f64 * 6.0) / (age + 0.5)
}

fn search_tokens(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn relevance(post: &Post, tokens: &[String]) -> usize {
    if tokens.is_empty() {
        return 0;
    }
    let haystack = format!(
        "{} {} {} {}",
        post.title,
        strip_markdown(&post.body),
        post.author,
        post.subreddit
    )
    .to_ascii_lowercase();
    tokens
        .iter()
        .map(|t| haystack.matches(t.as_str()).count())
        .sum()
}

fn hash(s: String) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[async_trait]
impl ContentProvider for DemoProvider {
    fn name(&self) -> &'static str {
        "Demo data"
    }

    fn short_name(&self) -> &'static str {
        "DEMO"
    }

    fn is_demo(&self) -> bool {
        true
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            search: true,
            search_in_subreddit: true,
            saved: false,
            images: true,
            oauth: false,
            demo: true,
        }
    }

    fn supported_sorts(&self, _spec: &FeedSpec) -> Vec<FeedSort> {
        FeedSort::ALL.to_vec()
    }

    fn resolve_media_url(&self, url: &str) -> String {
        if let Some(name) = url.strip_prefix("demo://img/")
            && let Some(dir) = &self.image_dir
        {
            let path = dir.join(name);
            if let Ok(url) = url::Url::from_file_path(&path) {
                return url.to_string();
            }
        }
        url.to_string()
    }

    async fn get_feed(&self, request: FeedRequest) -> Result<FeedPage, ProviderError> {
        if request.limit == 0 {
            return Err(ProviderError::InvalidInput("limit must be positive".into()));
        }

        let mut posts: Vec<Post> = match &request.spec {
            FeedSpec::Home => self.posts.clone(),
            FeedSpec::Popular => {
                let mut posts = self.posts.clone();
                posts.sort_by_key(|p| std::cmp::Reverse(p.score));
                posts.truncate(50);
                posts
            }
            FeedSpec::Subreddit(name) => {
                let name = normalize_subreddit(name)
                    .ok_or_else(|| ProviderError::InvalidInput("invalid subreddit name".into()))?;
                self.posts
                    .iter()
                    .filter(|p| p.subreddit.eq_ignore_ascii_case(&name))
                    .cloned()
                    .collect()
            }
            FeedSpec::Search { query, scope } => {
                let query = query.trim();
                if query.is_empty() {
                    return Err(ProviderError::InvalidInput("search query is empty".into()));
                }
                let tokens = search_tokens(query);
                self.posts
                    .iter()
                    .filter(|p| {
                        if let SearchScope::Subreddit(name) = scope
                            && !p.subreddit.eq_ignore_ascii_case(name)
                        {
                            return false;
                        }
                        relevance(p, &tokens) > 0
                    })
                    .cloned()
                    .collect()
            }
        };

        let query = match &request.spec {
            FeedSpec::Search { query, .. } => Some(query.as_str()),
            _ => None,
        };
        self.sort_posts(&mut posts, request.sort, request.top_range, query);

        let offset = request
            .after
            .as_deref()
            .and_then(parse_demo_cursor)
            .unwrap_or(0)
            .min(posts.len());

        let end = (offset + request.limit as usize).min(posts.len());
        let page_posts = posts[offset..end].to_vec();
        let has_more = end < posts.len();
        Ok(FeedPage {
            posts: page_posts,
            after: has_more.then(|| format!("d:{end}")),
            has_more,
        })
    }

    async fn get_post(&self, post_id: &str) -> Result<Post, ProviderError> {
        let id = post_id.trim_start_matches("t3_");
        self.posts
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(format!("post {id} is not in the demo data")))
    }

    async fn get_comments(
        &self,
        post_id: &str,
        sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        let mut tree = self
            .comments
            .get(post_id.trim_start_matches("t3_"))
            .cloned()
            .ok_or_else(|| {
                ProviderError::NotFound(format!("post {post_id} has no demo comments"))
            })?;
        for root in &mut tree {
            root.sort_tree(sort);
        }
        tree.sort_by(|a, b| match sort {
            CommentSort::New => b.created_utc.cmp(&a.created_utc),
            CommentSort::Old => a.created_utc.cmp(&b.created_utc),
            CommentSort::Controversial => a.score.unsigned_abs().cmp(&b.score.unsigned_abs()),
            _ => b.score.cmp(&a.score),
        });
        Ok(tree)
    }

    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, ProviderError> {
        let name = normalize_subreddit(name)
            .ok_or_else(|| ProviderError::InvalidInput("invalid subreddit name".into()))?;
        self.subreddits
            .get(&name)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(format!("r/{name} is not in the demo data")))
    }

    async fn get_more_comments(
        &self,
        post_id: &str,
        children: &[String],
        _sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        if children.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.make_more_comments(post_id, children))
    }
}

fn parse_demo_cursor(cursor: &str) -> Option<usize> {
    cursor.strip_prefix("d:")?.parse().ok()
}

const DEMO_REPLY_AUTHORS: [&str; 8] = [
    "helper_hazel",
    "terminal_tom",
    "quiet_lurker",
    "build_breaker",
    "open_source_otter",
    "kernel_kate",
    "byte_wanderer",
    "small_steps",
];

const DEMO_REPLY_BODIES: [&str; 8] = [
    "Adding a bit of context from my own setup: this worked on two machines and failed on the oldest one.",
    "The linked follow-up is worth reading too. It covers the edge cases this post skips.",
    "I had to change one flag, but after that it was smooth sailing.",
    "Does anyone know whether this still applies to the 2026 release?",
    "Thanks for taking the time to write this down. The examples made it click for me.",
    "We hit the same issue at work. The fix was boring and that is a compliment.",
    "I would love to see a follow-up with benchmarks on lower-end hardware.",
    "This is exactly the kind of practical detail that is missing from most guides.",
];

/// A small self-check used by the provider tests.
#[cfg(test)]
pub(crate) fn fixture_shapes() -> (usize, usize, usize) {
    let posts: Vec<serde_json::Value> = serde_json::from_str(POSTS_JSON).expect("posts json");
    let comments: serde_json::Value = serde_json::from_str(COMMENTS_JSON).expect("comments json");
    let subs: Vec<serde_json::Value> =
        serde_json::from_str(SUBREDDITS_JSON).expect("subreddits json");
    (
        posts.len(),
        comments.as_object().map(|m| m.len()).unwrap_or(0),
        subs.len(),
    )
}

/// Shared handle used by the UI and request tasks.
pub type ProviderHandle = Arc<dyn ContentProvider>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FeedSort;

    fn provider() -> DemoProvider {
        DemoProvider::new(&std::env::temp_dir().join("reddtty-demo-test"))
    }

    #[tokio::test]
    async fn demo_has_enough_posts_and_every_kind() {
        let p = provider();
        assert!(p.post_count() >= 30);
        let all = p
            .get_feed(FeedRequest::first(
                FeedSpec::Home,
                FeedSort::New,
                TopRange::All,
                100,
            ))
            .await
            .unwrap();
        assert!(all.posts.len() >= 30);
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Text))
        );
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Image { .. }))
        );
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Gallery { .. }))
        );
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Link { .. }))
        );
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Video { .. }))
        );
        assert!(
            all.posts
                .iter()
                .any(|p| matches!(p.content, crate::models::PostContent::Removed))
        );
    }

    #[tokio::test]
    async fn sorts_produce_different_orders() {
        let p = provider();
        let mut orders = Vec::new();
        for sort in [
            FeedSort::Hot,
            FeedSort::New,
            FeedSort::Top,
            FeedSort::Rising,
        ] {
            let page = p
                .get_feed(FeedRequest::first(FeedSpec::Home, sort, TopRange::All, 40))
                .await
                .unwrap();
            orders.push(page.posts.iter().map(|x| x.id.clone()).collect::<Vec<_>>());
        }
        assert_ne!(orders[0], orders[1], "hot and new must differ");
        assert_ne!(
            orders[1], orders[2],
            "new and top-all can legitimately differ"
        );
        assert_ne!(orders[0], orders[3], "hot and rising must differ");
    }

    #[tokio::test]
    async fn top_time_filter_changes_results() {
        let p = provider();
        let day = p
            .get_feed(FeedRequest::first(
                FeedSpec::Home,
                FeedSort::Top,
                TopRange::Day,
                100,
            ))
            .await
            .unwrap();
        let all = p
            .get_feed(FeedRequest::first(
                FeedSpec::Home,
                FeedSort::Top,
                TopRange::All,
                100,
            ))
            .await
            .unwrap();
        assert!(day.posts.len() < all.posts.len());
        assert!(
            day.posts
                .iter()
                .all(|post| p.now - post.created_utc <= 86_400)
        );
    }

    #[tokio::test]
    async fn pagination_is_cursor_based_and_does_not_duplicate() {
        let p = provider();
        let first = p
            .get_feed(FeedRequest::first(
                FeedSpec::Home,
                FeedSort::Hot,
                TopRange::All,
                10,
            ))
            .await
            .unwrap();
        assert!(first.has_more);
        let cursor = first.after.clone().unwrap();
        let second = p
            .get_feed(FeedRequest {
                spec: FeedSpec::Home,
                sort: FeedSort::Hot,
                top_range: TopRange::All,
                after: Some(cursor),
                limit: 10,
            })
            .await
            .unwrap();
        let ids: std::collections::HashSet<_> = first.posts.iter().map(|x| x.id.clone()).collect();
        assert!(second.posts.iter().all(|post| !ids.contains(&post.id)));
    }

    #[tokio::test]
    async fn search_filters_by_scope() {
        let p = provider();
        let all = p
            .get_feed(FeedRequest::first(
                FeedSpec::Search {
                    query: "rust".into(),
                    scope: SearchScope::All,
                },
                FeedSort::Best,
                TopRange::All,
                100,
            ))
            .await
            .unwrap();
        assert!(!all.posts.is_empty());
        let scoped = p
            .get_feed(FeedRequest::first(
                FeedSpec::Search {
                    query: "linux".into(),
                    scope: SearchScope::Subreddit("linux".into()),
                },
                FeedSort::Best,
                TopRange::All,
                100,
            ))
            .await
            .unwrap();
        assert!(scoped.posts.iter().all(|post| post.subreddit == "linux"));
    }

    #[tokio::test]
    async fn comments_are_nested_and_sortable() {
        let p = provider();
        let best = p.get_comments("demo001", CommentSort::Best).await.unwrap();
        assert!(best.iter().any(|c| !c.replies.is_empty()));
        assert!(best.iter().any(|c| c.descendant_count() >= 3));
        let new = p.get_comments("demo001", CommentSort::New).await.unwrap();
        assert!(!new.is_empty());
    }

    #[tokio::test]
    async fn more_comments_can_be_loaded() {
        let p = provider();
        let comments = p.get_comments("demo001", CommentSort::Best).await.unwrap();
        let more = comments
            .iter()
            .flat_map(|c| c.replies.iter())
            .find(|c| c.is_more)
            .expect("demo001 should have a more stub");
        assert!(!more.more_children.is_empty());
        let loaded = p
            .get_more_comments("demo001", &more.more_children, CommentSort::Best)
            .await
            .unwrap();
        assert_eq!(loaded.len(), more.more_children.len());
        assert!(loaded.iter().any(|c| !c.replies.is_empty()));
    }

    #[tokio::test]
    async fn unknown_subreddit_errors_cleanly() {
        let p = provider();
        let err = p.get_subreddit("notarealsub").await.unwrap_err();
        assert!(matches!(err, ProviderError::NotFound(_)));
        let err = p
            .get_subreddit("this-name-is-invalid!!!")
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::InvalidInput(_)));
        let page = p
            .get_feed(FeedRequest::first(
                FeedSpec::Subreddit("notarealsub".into()),
                FeedSort::Hot,
                TopRange::All,
                25,
            ))
            .await
            .unwrap();
        assert!(page.posts.is_empty());
    }

    #[tokio::test]
    async fn media_urls_resolve_to_local_files() {
        let p = provider();
        let resolved = p.resolve_media_url("demo://img/aurora.png");
        assert!(
            resolved.starts_with("file://"),
            "demo image should resolve to a local file URL, got {resolved}"
        );
        let path = url::Url::parse(&resolved)
            .expect("valid file URL")
            .to_file_path()
            .expect("file URL path");
        assert!(path.exists(), "resolved demo image is missing: {path:?}");
        assert_eq!(
            p.resolve_media_url("https://example.com/x.png"),
            "https://example.com/x.png"
        );
    }

    #[test]
    fn fixtures_have_expected_shapes() {
        let (posts, comments, subs) = fixture_shapes();
        assert!(posts >= 30);
        assert!(comments >= 30);
        assert!(subs >= 5);
    }
}
