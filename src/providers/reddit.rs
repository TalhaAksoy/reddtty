//! Live Reddit provider.
//!
//! Reddit requires OAuth for Data API access, and since the Responsible
//! Builder Policy took effect new OAuth clients need approval before they can
//! be used. This provider therefore never scrapes, never reuses browser
//! cookies, and never falls back to unauthenticated endpoints. If credentials
//! or approval are missing, it returns an explanatory [`ProviderError`].
//!
//! Only `GET` read endpoints are used. Writes (voting, commenting, saving)
//! are intentionally out of scope.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::Mutex as AsyncMutex;

use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde_json::Value;

use crate::error::ProviderError;
use crate::models::{
    Comment, CommentSort, FeedPage, FeedRequest, FeedSort, FeedSpec, MediaItem, Post, PostContent,
    SearchScope, Subreddit,
};
use crate::providers::traits::{ContentProvider, ProviderCapabilities};

const DEFAULT_BASE_URL: &str = "https://oauth.reddit.com";
const DEFAULT_TOKEN_URL: &str = "https://www.reddit.com/api/v1/access_token";
const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug)]
pub struct RedditConfig {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: Option<String>,
    pub username: Option<String>,
    pub user_agent: String,
    /// Overridable for tests and for self-hosted proxies. Must speak the
    /// documented Reddit OAuth API.
    pub base_url: String,
    pub token_url: String,
    pub request_timeout: Duration,
}

impl RedditConfig {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        user_agent: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            refresh_token: None,
            username: None,
            user_agent: user_agent.into(),
            base_url: DEFAULT_BASE_URL.into(),
            token_url: DEFAULT_TOKEN_URL.into(),
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    pub fn with_endpoints(
        mut self,
        base_url: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        self.base_url = base_url.into();
        self.token_url = token_url.into();
        self
    }
}

#[derive(Clone, Debug)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Debug, Default)]
struct RateState {
    blocked_until: Option<Instant>,
    remaining: Option<u32>,
    reset_seconds: u64,
}

pub struct RedditProvider {
    http: reqwest::Client,
    config: RedditConfig,
    token: Mutex<Option<CachedToken>>,
    token_refresh: AsyncMutex<()>,
    more_children: AsyncMutex<()>,
    rate: Mutex<RateState>,
}

impl RedditProvider {
    pub fn new(config: RedditConfig) -> Result<Self, ProviderError> {
        if config.client_id.trim().is_empty() || config.client_secret.trim().is_empty() {
            return Err(ProviderError::AuthRequired(
                "Set REDDTTY_REDDIT_CLIENT_ID and REDDTTY_REDDIT_CLIENT_SECRET, or add them to the config file.".into(),
            ));
        }
        if !config.user_agent.to_ascii_lowercase().contains("reddtty") {
            return Err(ProviderError::InvalidInput(
                "user_agent must identify ReddTTY (Reddit requires a descriptive, unique User-Agent).".into(),
            ));
        }

        let http = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.request_timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| ProviderError::Network(format!("could not build HTTP client: {e}")))?;

        Ok(Self {
            http,
            config,
            token: Mutex::new(None),
            token_refresh: AsyncMutex::new(()),
            more_children: AsyncMutex::new(()),
            rate: Mutex::new(RateState::default()),
        })
    }

    async fn access_token(&self) -> Result<String, ProviderError> {
        if let Some(token) = self.token.lock().ok().and_then(|guard| guard.clone())
            && token.expires_at > Instant::now() + Duration::from_secs(30)
        {
            return Ok(token.access_token);
        }

        // Serialise refreshes so concurrent feed/about/comment requests do not
        // each hit the token endpoint.
        let _refresh_guard = self.token_refresh.lock().await;
        if let Some(token) = self.token.lock().ok().and_then(|guard| guard.clone())
            && token.expires_at > Instant::now() + Duration::from_secs(30)
        {
            return Ok(token.access_token);
        }

        let mut form: Vec<(&str, String)> = Vec::new();
        if let Some(refresh) = &self.config.refresh_token {
            form.push(("grant_type", "refresh_token".into()));
            form.push(("refresh_token", refresh.clone()));
        } else {
            form.push(("grant_type", "client_credentials".into()));
            form.push(("scope", "read".into()));
        }

        let response = self
            .http
            .post(&self.config.token_url)
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&form)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let headers = response.headers().clone();
        let body = read_body(response).await?;

        if !status.is_success() {
            self.note_rate_headers(&headers);
            return Err(match status.as_u16() {
                401 | 403 => ProviderError::AuthRequired(
                    "Reddit rejected the app credentials. Confirm the app is registered and approved under the Responsible Builder Policy, and that the refresh token (if used) matches the app.".into(),
                ),
                429 => ProviderError::RateLimited {
                    retry_after: retry_after(&headers),
                },
                _ => ProviderError::from_status(status.as_u16(), &body, retry_after(&headers)),
            });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| ProviderError::Parse(format!("token response was not JSON: {e}")))?;
        if let Some(error) = json.get("error").and_then(Value::as_str) {
            let description = json
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or("no description");
            return Err(ProviderError::AuthRequired(format!(
                "Reddit token endpoint returned `{error}` ({description})."
            )));
        }
        let access_token = json
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::Parse("token response had no access_token".into()))?
            .to_string();
        let expires_in = json
            .get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(3600)
            .max(60);
        let cached = CachedToken {
            access_token: access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expires_in.saturating_sub(60)),
        };
        if let Ok(mut guard) = self.token.lock() {
            *guard = Some(cached);
        }
        Ok(access_token)
    }

    fn invalidate_token(&self) {
        if let Ok(mut guard) = self.token.lock() {
            *guard = None;
        }
    }

    fn check_rate_limit(&self) -> Result<(), ProviderError> {
        let Ok(mut rate) = self.rate.lock() else {
            return Ok(());
        };
        if let Some(until) = rate.blocked_until {
            let now = Instant::now();
            if until > now {
                return Err(ProviderError::RateLimited {
                    retry_after: Some((until - now).as_secs().max(1)),
                });
            }
            rate.blocked_until = None;
        }
        Ok(())
    }

    fn note_rate_headers(&self, headers: &HeaderMap) {
        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<f64>().ok());
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(60);
        if let Ok(mut rate) = self.rate.lock() {
            rate.remaining = remaining.map(|r| r.max(0.0) as u32);
            rate.reset_seconds = reset;
            if remaining.is_some_and(|r| r <= 0.0) {
                rate.blocked_until = Some(Instant::now() + Duration::from_secs(reset.max(1)));
            }
        }
    }

    async fn get_json(&self, path: &str, query: &[(&str, String)]) -> Result<Value, ProviderError> {
        self.check_rate_limit()?;

        let mut last_auth_error = None;
        for attempt in 0..2 {
            let token = match self.access_token().await {
                Ok(token) => token,
                Err(err) => return Err(err),
            };
            let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
            let response = self
                .http
                .get(url)
                .bearer_auth(&token)
                .header("Accept", "application/json")
                .query(query)
                .send()
                .await
                .map_err(map_reqwest_error)?;

            let status = response.status();
            let headers = response.headers().clone();
            let body = read_body(response).await?;
            self.note_rate_headers(&headers);

            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                // Token expiry is the common cause. Refresh once, then give up.
                self.invalidate_token();
                last_auth_error = Some(ProviderError::AuthRequired(
                    "Reddit rejected the access token. Check your credentials and app approval."
                        .into(),
                ));
                continue;
            }

            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => ProviderError::AuthRequired(
                        "Reddit rejected the access token after a refresh. Confirm app approval and scopes (read).".into(),
                    ),
                    403 => ProviderError::Forbidden(
                        "Reddit refused access. The app may not be approved for this endpoint, or the content is private/quarantined.".into(),
                    ),
                    404 => ProviderError::NotFound(format!("Reddit returned 404 for {path}")),
                    429 => {
                        let retry = retry_after(&headers);
                        if let Ok(mut rate) = self.rate.lock() {
                            rate.blocked_until = Some(
                                Instant::now() + Duration::from_secs(retry.unwrap_or(60).max(1)),
                            );
                        }
                        ProviderError::RateLimited { retry_after: retry }
                    }
                    s => ProviderError::from_status(s, &body, retry_after(&headers)),
                });
            }

            return serde_json::from_str::<Value>(&body)
                .map_err(|e| ProviderError::Parse(format!("Reddit returned invalid JSON: {e}")));
        }

        Err(last_auth_error
            .unwrap_or_else(|| ProviderError::AuthRequired("Reddit authentication failed.".into())))
    }

    fn listing_path(&self, request: &FeedRequest) -> Result<String, ProviderError> {
        let sort = request.sort.api_value_for(&request.spec);
        let path = match &request.spec {
            FeedSpec::Home => format!("/{sort}"),
            FeedSpec::Popular => format!("/r/popular/{sort}"),
            FeedSpec::Subreddit(name) => format!("/r/{}/{sort}", encode_path_segment(name)),
            FeedSpec::Search { scope, .. } => match scope {
                SearchScope::All => "/search".to_string(),
                SearchScope::Subreddit(name) => {
                    format!("/r/{}/search", encode_path_segment(name))
                }
            },
        };
        Ok(path)
    }

    fn feed_query(&self, request: &FeedRequest) -> Vec<(&'static str, String)> {
        let mut query: Vec<(&str, String)> = vec![
            ("raw_json", "1".into()),
            ("limit", request.limit.clamp(1, 100).to_string()),
        ];
        if let Some(after) = &request.after {
            query.push(("after", after.clone()));
        }

        if let FeedSpec::Search { query: q, scope } = &request.spec {
            query.push(("q", q.clone()));
            query.push(("type", "link".into()));
            query.push((
                "sort",
                request.sort.api_value_for(&request.spec).to_string(),
            ));
            if matches!(scope, SearchScope::Subreddit(_)) {
                query.push(("restrict_sr", "1".into()));
            }
        }

        if matches!(request.sort, FeedSort::Top | FeedSort::Controversial)
            || matches!(&request.spec, FeedSpec::Search { .. })
        {
            query.push(("t", request.top_range.api_value().into()));
        }
        query
    }
}

#[async_trait]
impl ContentProvider for RedditProvider {
    fn name(&self) -> &'static str {
        "Reddit (OAuth)"
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
            oauth: true,
            demo: false,
        }
    }

    async fn get_feed(&self, request: FeedRequest) -> Result<FeedPage, ProviderError> {
        if let FeedSpec::Search { query, .. } = &request.spec
            && query.trim().is_empty()
        {
            return Err(ProviderError::InvalidInput("search query is empty".into()));
        }
        let path = self.listing_path(&request)?;
        let query = self.feed_query(&request);
        let json = self.get_json(&path, &query).await?;
        let data = json
            .get("data")
            .ok_or_else(|| ProviderError::Parse("listing had no data object".into()))?;
        let children = data
            .get("children")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Parse("listing had no children array".into()))?;

        let posts = children
            .iter()
            .filter(|child| child.get("kind").and_then(Value::as_str) == Some("t3"))
            .filter_map(|child| child.get("data"))
            .map(map_post)
            .collect::<Vec<_>>();

        let after = data
            .get("after")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        Ok(FeedPage {
            has_more: after.is_some(),
            after,
            posts,
        })
    }

    async fn get_post(&self, post_id: &str) -> Result<Post, ProviderError> {
        let id = post_id.trim_start_matches("t3_");
        let json = self
            .get_json(
                &format!("/comments/{}", encode_path_segment(id)),
                &[("raw_json", "1".into()), ("limit", "0".into())],
            )
            .await?;
        let post = json
            .get(0)
            .and_then(|v| v.get("data"))
            .and_then(|v| v.get("children"))
            .and_then(Value::as_array)
            .and_then(|children| {
                children
                    .iter()
                    .find(|c| c.get("kind").and_then(Value::as_str) == Some("t3"))
            })
            .and_then(|c| c.get("data"))
            .ok_or_else(|| ProviderError::NotFound(format!("post {id} was not returned")))?;
        Ok(map_post(post))
    }

    async fn get_comments(
        &self,
        post_id: &str,
        sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        let id = post_id.trim_start_matches("t3_");
        let json = self
            .get_json(
                &format!("/comments/{}", encode_path_segment(id)),
                &[
                    ("raw_json", "1".into()),
                    ("sort", sort.api_value().into()),
                    ("limit", "100".into()),
                    ("depth", "8".into()),
                ],
            )
            .await?;
        let roots = json
            .get(1)
            .and_then(|v| v.get("data"))
            .and_then(|v| v.get("children"))
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Parse("comments listing was missing".into()))?;
        Ok(roots.iter().filter_map(map_comment).collect())
    }

    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, ProviderError> {
        let path = format!("/r/{}/about", encode_path_segment(name));
        let json = self.get_json(&path, &[("raw_json", "1".into())]).await?;
        let data = json
            .get("data")
            .ok_or_else(|| ProviderError::Parse("subreddit response had no data".into()))?;
        Ok(Subreddit {
            name: json_str(data, "display_name").unwrap_or_else(|| name.to_string()),
            title: json_str(data, "title").unwrap_or_default(),
            description: json_str(data, "public_description").unwrap_or_default(),
            subscribers: json_u64(data, "subscribers"),
            active_users: json_u64(data, "active_user_count"),
            over18: json_bool(data, "over18"),
            created_utc: json_i64(data, "created_utc"),
        })
    }

    async fn get_more_comments(
        &self,
        post_id: &str,
        children: &[String],
        sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        if children.is_empty() {
            return Ok(Vec::new());
        }
        // Reddit documents that only one morechildren request may run at a time.
        let _one_at_a_time = self.more_children.lock().await;
        let link_id = if post_id.starts_with("t3_") {
            post_id.to_string()
        } else {
            format!("t3_{post_id}")
        };
        let json = self
            .get_json(
                "/api/morechildren",
                &[
                    ("api_type", "json".into()),
                    ("link_id", link_id),
                    ("children", children.join(",")),
                    ("sort", sort.api_value().into()),
                    ("raw_json", "1".into()),
                    ("limit_children", "false".into()),
                ],
            )
            .await?;
        let things = json
            .get("json")
            .and_then(|v| v.get("data"))
            .and_then(|v| v.get("things"))
            .and_then(Value::as_array)
            .or_else(|| {
                json.get("data")
                    .and_then(|v| v.get("things"))
                    .and_then(Value::as_array)
            })
            .ok_or_else(|| {
                ProviderError::Parse("morechildren response had no things array".into())
            })?;

        let mut mapped = Vec::new();
        for thing in things {
            match thing.get("kind").and_then(Value::as_str) {
                Some("t1") => {
                    if let Some(data) = thing.get("data") {
                        mapped.push(map_comment_data(data));
                    }
                }
                Some("more") => {
                    if let Some(data) = thing.get("data") {
                        mapped.push(map_more(data));
                    }
                }
                _ => {}
            }
        }
        Ok(build_comment_forest(mapped))
    }
}

fn map_post(data: &Value) -> Post {
    let url = json_str(data, "url_overridden_by_dest")
        .or_else(|| json_str(data, "url"))
        .unwrap_or_default();
    let is_self = json_bool(data, "is_self");
    let is_gallery = json_bool(data, "is_gallery");
    let is_video = json_bool(data, "is_video");
    let hint = json_str(data, "post_hint").unwrap_or_default();
    let thumbnail = best_preview(data);

    let removed_by = json_str(data, "removed_by_category")
        .or_else(|| json_str(data, "removed_by"))
        .unwrap_or_default();
    let selftext = json_str(data, "selftext").unwrap_or_default();
    let author = json_str(data, "author").unwrap_or_else(|| "[deleted]".into());
    let title = json_str(data, "title").unwrap_or_else(|| "[removed]".into());
    let removed = !removed_by.is_empty()
        || selftext == "[removed]"
        || title == "[removed]"
        || title == "[deleted]";

    let content = if removed {
        PostContent::Removed
    } else if is_gallery {
        let items = gallery_items(data);
        if items.is_empty() {
            PostContent::Link {
                url: url.clone(),
                thumbnail: thumbnail.clone(),
            }
        } else {
            PostContent::Gallery { items }
        }
    } else if is_video {
        let video_url = data
            .get("media")
            .and_then(|m| m.get("reddit_video"))
            .and_then(|v| v.get("fallback_url"))
            .and_then(Value::as_str)
            .map(html_unescape)
            .unwrap_or_else(|| url.clone());
        PostContent::Video {
            url: video_url,
            thumbnail: thumbnail.clone(),
            duration: data
                .get("media")
                .and_then(|m| m.get("reddit_video"))
                .and_then(|v| v.get("duration"))
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        }
    } else if hint == "image" || is_image_url(&url) {
        PostContent::Image { url: url.clone() }
    } else if !is_self && !url.is_empty() {
        PostContent::Link {
            url: url.clone(),
            thumbnail: thumbnail.clone(),
        }
    } else {
        PostContent::Text
    };

    let body = if removed || selftext == "[removed]" || selftext == "[deleted]" {
        String::new()
    } else {
        selftext
    };

    Post {
        id: json_str(data, "id").unwrap_or_default(),
        subreddit: json_str(data, "subreddit").unwrap_or_default(),
        author,
        title,
        body,
        created_utc: json_i64(data, "created_utc"),
        score: json_i64(data, "score"),
        num_comments: json_u64(data, "num_comments").min(u32::MAX as u64) as u32,
        permalink: json_str(data, "permalink").unwrap_or_default(),
        content,
        flair: json_str(data, "link_flair_text").filter(|s| !s.is_empty()),
        nsfw: json_bool(data, "over_18"),
        spoiler: json_bool(data, "spoiler"),
        stickied: json_bool(data, "stickied"),
        locked: json_bool(data, "locked"),
        removed,
        is_self,
        domain: json_str(data, "domain").filter(|d| !d.is_empty()),
        thumbnail,
    }
}

fn gallery_items(data: &Value) -> Vec<MediaItem> {
    let gallery = data.get("gallery_data").and_then(|g| g.get("items"));
    let metadata = data.get("media_metadata");
    let mut out = Vec::new();

    if let (Some(items), Some(metadata)) = (
        gallery.and_then(Value::as_array),
        metadata.and_then(Value::as_object),
    ) {
        for item in items {
            let Some(media_id) = item.get("media_id").and_then(Value::as_str) else {
                continue;
            };
            let Some(meta) = metadata.get(media_id) else {
                continue;
            };
            let url = meta
                .get("s")
                .and_then(|s| {
                    s.get("u")
                        .or_else(|| s.get("gif"))
                        .or_else(|| s.get("mp4"))
                        .and_then(Value::as_str)
                })
                .map(html_unescape);
            if let Some(url) = url {
                let width = meta
                    .get("s")
                    .and_then(|s| s.get("x"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32);
                let height = meta
                    .get("s")
                    .and_then(|s| s.get("y"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32);
                out.push(MediaItem { url, width, height });
            }
        }
    }
    out
}

fn best_preview(data: &Value) -> Option<String> {
    if let Some(first) = data
        .get("preview")
        .and_then(|p| p.get("images"))
        .and_then(Value::as_array)
        .and_then(|images| images.first())
    {
        if let Some(url) = first
            .get("source")
            .and_then(|s| s.get("url"))
            .and_then(Value::as_str)
        {
            return Some(html_unescape(url));
        }
        if let Some(url) = first
            .get("resolutions")
            .and_then(Value::as_array)
            .and_then(|resolutions| resolutions.last())
            .and_then(|r| r.get("url"))
            .and_then(Value::as_str)
        {
            return Some(html_unescape(url));
        }
    }
    json_str(data, "thumbnail").filter(|t| {
        !t.is_empty()
            && !matches!(
                t.as_str(),
                "self" | "default" | "nsfw" | "spoiler" | "image"
            )
            && (t.starts_with("http://") || t.starts_with("https://"))
    })
}

fn is_image_url(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

fn map_comment(thing: &Value) -> Option<Comment> {
    match thing.get("kind").and_then(Value::as_str) {
        Some("t1") => thing.get("data").map(map_comment_data),
        Some("more") => thing.get("data").map(map_more),
        _ => None,
    }
}

fn map_comment_data(data: &Value) -> Comment {
    let body = json_str(data, "body").unwrap_or_default();
    let author = json_str(data, "author").unwrap_or_else(|| "[deleted]".into());
    let mut comment = Comment {
        id: json_str(data, "id")
            .or_else(|| json_str(data, "name"))
            .unwrap_or_default(),
        parent_id: json_str(data, "parent_id"),
        author,
        body,
        score: json_i64(data, "score"),
        created_utc: json_i64(data, "created_utc"),
        distinguished: json_str(data, "distinguished").filter(|d| !d.is_empty()),
        deleted: json_bool(data, "deleted"),
        removed: json_bool(data, "removed"),
        is_more: false,
        more_count: 0,
        more_children: Vec::new(),
        replies: Vec::new(),
    };
    if comment.author == "[deleted]" || comment.body == "[deleted]" {
        comment.deleted = true;
    }
    if comment.body == "[removed]" {
        comment.removed = true;
    }
    if let Some(replies) = data.get("replies")
        && replies.is_object()
        && let Some(children) = replies
            .get("data")
            .and_then(|d| d.get("children"))
            .and_then(Value::as_array)
    {
        comment.replies = children.iter().filter_map(map_comment).collect();
    }
    comment
}

fn map_more(data: &Value) -> Comment {
    let parent_id = json_str(data, "parent_id");
    let more_children: Vec<String> = data
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let more_count = json_u64(data, "count").min(u32::MAX as u64) as u32;
    // Reddit can reuse short ids such as "_" for several MoreComments stubs.
    // Fullnames are unique; fall back to a synthetic id that includes parent
    // and the first child id so UI selection/loading cannot collide.
    let id = json_str(data, "name")
        .filter(|name| !name.is_empty() && name != "t1__")
        .or_else(|| json_str(data, "id").filter(|id| !id.is_empty() && id != "_"))
        .unwrap_or_else(|| {
            format!(
                "more:{}:{}:{}",
                parent_id.as_deref().unwrap_or("root"),
                more_children.first().map(String::as_str).unwrap_or("none"),
                more_count
            )
        });
    Comment {
        id,
        parent_id,
        author: "[more]".into(),
        body: String::new(),
        score: 0,
        created_utc: 0,
        distinguished: None,
        deleted: false,
        removed: false,
        is_more: true,
        more_count,
        more_children,
        replies: Vec::new(),
    }
}

/// Rebuild a forest from the flat list returned by `/api/morechildren`.
fn build_comment_forest(comments: Vec<Comment>) -> Vec<Comment> {
    let mut by_id: std::collections::HashMap<String, Comment> =
        comments.iter().map(|c| (c.id.clone(), c.clone())).collect();
    let mut children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut roots = Vec::new();

    for comment in &comments {
        let parent = comment
            .parent_id
            .as_deref()
            .and_then(|p| p.strip_prefix("t1_"))
            .filter(|p| *p != comment.id && by_id.contains_key(*p));
        match parent {
            Some(parent) => children
                .entry(parent.to_string())
                .or_default()
                .push(comment.id.clone()),
            None => roots.push(comment.id.clone()),
        }
    }

    roots
        .into_iter()
        .filter_map(|id| take_node(&id, &mut by_id, &children, 0))
        .collect()
}

fn take_node(
    id: &str,
    by_id: &mut std::collections::HashMap<String, Comment>,
    children: &std::collections::HashMap<String, Vec<String>>,
    depth: usize,
) -> Option<Comment> {
    let mut node = by_id.remove(id)?;
    if depth < 32
        && let Some(kids) = children.get(id)
    {
        node.replies = kids
            .iter()
            .filter_map(|kid| take_node(kid, by_id, children, depth + 1))
            .collect();
    }
    Some(node)
}

async fn read_body(response: reqwest::Response) -> Result<String, ProviderError> {
    if let Some(len) = response.content_length()
        && len > MAX_JSON_BYTES as u64
    {
        return Err(ProviderError::Parse(format!(
            "response too large ({len} bytes)"
        )));
    }
    let status = response.status();
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
        if bytes.len() + chunk.len() > MAX_JSON_BYTES {
            return Err(ProviderError::Parse("response exceeded size limit".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes)
        .map_err(|_| ProviderError::Parse(format!("response for HTTP {status} was not UTF-8")))
}

fn map_reqwest_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout(REQUEST_TIMEOUT.as_secs())
    } else if err.is_connect() {
        ProviderError::Network("could not connect to Reddit".into())
    } else if err.is_request() {
        ProviderError::Network(format!("request failed: {err}"))
    } else {
        ProviderError::Network(err.to_string())
    }
}

fn retry_after(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .or_else(|| {
            headers
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<f64>().ok())
                .map(|v| v.max(0.0) as u64)
        })
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn json_bool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn json_i64(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0)
}

fn json_u64(v: &Value, key: &str) -> u64 {
    v.get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f.max(0.0) as u64)))
        .unwrap_or(0)
}

fn encode_path_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn html_unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let rest = &input[index + 1..];
        let end = rest.find(';');
        let Some(end) = end else {
            out.push('&');
            continue;
        };
        let entity = &rest[..end];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            out.push(replacement);
            // Skip the entity and semicolon.
            for _ in 0..(end + 1) {
                chars.next();
            }
        } else {
            out.push('&');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TopRange;

    #[test]
    fn unescapes_html_entities() {
        assert_eq!(
            html_unescape("https://x.test/a?b=1&amp;c=2"),
            "https://x.test/a?b=1&c=2"
        );
        assert_eq!(html_unescape("&#x2F;r&#x2F;rust"), "/r/rust");
        assert_eq!(html_unescape("&quot;hi&quot;"), "\"hi\"");
    }

    #[test]
    fn maps_text_post() {
        let data = serde_json::json!({
            "id": "abc", "subreddit": "rust", "author": "alice", "title": "Hello",
            "selftext": "Body", "created_utc": 1700000000.0, "score": 42,
            "num_comments": 7, "permalink": "/r/rust/comments/abc/hello/",
            "is_self": true, "over_18": false, "spoiler": false
        });
        let post = map_post(&data);
        assert_eq!(post.content, PostContent::Text);
        assert_eq!(post.score, 42);
        assert_eq!(post.num_comments, 7);
    }

    #[test]
    fn maps_gallery_in_order() {
        let data = serde_json::json!({
            "id": "g", "subreddit": "pics", "author": "b", "title": "G", "is_gallery": true,
            "gallery_data": {"items": [{"media_id": "one"}, {"media_id": "two"}]},
            "media_metadata": {
                "one": {"s": {"u": "https://i.test/1.png", "x": 10, "y": 20}},
                "two": {"s": {"u": "https://i.test/2.png"}}
            }
        });
        let post = map_post(&data);
        let PostContent::Gallery { items } = post.content else {
            panic!("expected gallery");
        };
        assert_eq!(items[0].url, "https://i.test/1.png");
        assert_eq!(items[0].width, Some(10));
        assert_eq!(items[1].url, "https://i.test/2.png");
    }

    #[test]
    fn maps_link_and_video() {
        let link = serde_json::json!({
            "id": "l", "subreddit": "x", "author": "b", "title": "L", "is_self": false,
            "url": "https://example.com/a", "domain": "example.com",
            "preview": {"images": [{"source": {"url": "https://i.test/p.jpg?a=1&amp;b=2"}}]}
        });
        let post = map_post(&link);
        let PostContent::Link { url, thumbnail } = post.content else {
            panic!("expected link");
        };
        assert_eq!(url, "https://example.com/a");
        assert_eq!(thumbnail.unwrap(), "https://i.test/p.jpg?a=1&b=2");

        let video = serde_json::json!({
            "id": "v", "subreddit": "x", "author": "b", "title": "V", "is_video": true,
            "url": "https://v.test/page", "media": {"reddit_video": {"fallback_url": "https://v.test/v.mp4", "duration": 12}}
        });
        let post = map_post(&video);
        assert!(matches!(
            post.content,
            PostContent::Video {
                duration: Some(12),
                ..
            }
        ));
    }

    #[test]
    fn maps_removed_post() {
        let data = serde_json::json!({
            "id": "r", "subreddit": "x", "author": "[deleted]", "title": "[removed]",
            "removed_by_category": "moderator", "is_self": true
        });
        let post = map_post(&data);
        assert!(post.removed);
        assert_eq!(post.content, PostContent::Removed);
    }

    #[test]
    fn maps_nested_comments() {
        let data = serde_json::json!({
            "id": "c1", "author": "a", "body": "root", "score": 5,
            "replies": {"data": {"children": [
                {"kind": "t1", "data": {"id": "c2", "parent_id": "t1_c1", "author": "b", "body": "child", "score": 1}},
                {"kind": "more", "data": {"id": "m", "count": 3, "children": ["x", "y", "z"], "parent_id": "t1_c1"}}
            ]}}
        });
        let comment = map_comment_data(&data);
        assert_eq!(comment.replies.len(), 2);
        assert_eq!(comment.replies[1].more_count, 3);
        assert!(comment.replies[1].is_more);
    }

    #[test]
    fn builds_forest_from_morechildren() {
        let comments = vec![
            Comment {
                id: "a".into(),
                parent_id: Some("t1_moreparent".into()),
                ..Comment::new("a", "x", "root")
            },
            Comment {
                id: "b".into(),
                parent_id: Some("t1_a".into()),
                ..Comment::new("b", "y", "child")
            },
        ];
        let forest = build_comment_forest(comments);
        assert_eq!(forest.len(), 1);
        assert_eq!(forest[0].replies.len(), 1);
    }

    #[test]
    fn path_and_query_are_built() {
        let provider =
            RedditProvider::new(RedditConfig::new("id", "secret", "reddtty/test")).unwrap();
        let request = FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::Top,
            TopRange::Week,
            25,
        );
        assert_eq!(provider.listing_path(&request).unwrap(), "/r/rust/top");
        let query = provider.feed_query(&request);
        assert!(query.contains(&("t", "week".to_string())));
        assert!(query.contains(&("raw_json", "1".to_string())));
    }

    #[test]
    fn unsupported_sorts_follow_reddit_rules() {
        assert!(!FeedSpec::Subreddit("rust".into()).supports_sort(FeedSort::Best));
        assert!(
            FeedSpec::Search {
                query: "x".into(),
                scope: SearchScope::All
            }
            .supports_sort(FeedSort::Best)
        );
    }
}
