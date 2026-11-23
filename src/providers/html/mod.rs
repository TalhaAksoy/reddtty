//! HTML-based read-only providers.
//!
//! These providers parse the public HTML of Old Reddit or a user-selected
//! Redlib instance. They are **not** Reddit's Data API, do not authenticate,
//! and do not attempt to bypass login walls, CAPTCHAs or rate limits.
//!
//! Network access is disabled unless `provider.html.allow_network = true` is
//! explicitly configured. The parsers are exercised by local fixtures and mock
//! HTTP servers; live access is never claimed without a real successful run.

pub mod client;
pub mod old_reddit;
pub mod parser;
pub mod redlib;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use url::Url;

use crate::error::ProviderError;
use crate::models::{CommentSort, FeedRequest, FeedSort, FeedSpec, Post, Subreddit};
use crate::utils::text::normalize_subreddit;

pub use client::{HtmlClient, HtmlClientConfig};
pub use old_reddit::OldRedditProvider;
pub use redlib::RedlibProvider;

pub const HTML_TIMEOUT: Duration = Duration::from_secs(20);

/// Sorts honestly supported by the HTML endpoints we build URLs for.
///
/// `Best` is only exposed for search, where it maps to Reddit's `relevance`
/// sort. It is not offered for listings, because those pages do not have a
/// documented "best" route.
pub(crate) fn sort_options(spec: &FeedSpec) -> Vec<FeedSort> {
    match spec {
        FeedSpec::Search { .. } => vec![
            FeedSort::Best,
            FeedSort::Hot,
            FeedSort::New,
            FeedSort::Top,
            FeedSort::Controversial,
        ],
        FeedSpec::Popular => vec![FeedSort::Hot, FeedSort::New, FeedSort::Top],
        FeedSpec::Home | FeedSpec::Subreddit(_) => vec![
            FeedSort::Hot,
            FeedSort::New,
            FeedSort::Top,
            FeedSort::Rising,
            FeedSort::Controversial,
        ],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HtmlFlavor {
    OldReddit,
    Redlib,
}

/// Build the first-page URL for a feed. Pagination uses the source's own
/// `next` link instead of reimplementing its cursor rules.
pub(crate) fn listing_url(
    base: &Url,
    request: &FeedRequest,
    flavor: HtmlFlavor,
) -> Result<Url, ProviderError> {
    match &request.spec {
        FeedSpec::Search { query, scope } => {
            let query = query.trim();
            if query.is_empty() {
                return Err(ProviderError::InvalidInput("search query is empty".into()));
            }
            let mut url = if let Some(name) = scope.subreddit() {
                let name = normalize_subreddit(name)
                    .ok_or_else(|| ProviderError::InvalidInput("invalid subreddit name".into()))?;
                base.join(&format!("/r/{name}/search"))?
            } else {
                base.join("/search")?
            };
            {
                let mut pairs = url.query_pairs_mut();
                pairs.append_pair("q", query);
                pairs.append_pair(
                    "sort",
                    request.sort.api_value_for(&FeedSpec::Search {
                        query: String::new(),
                        scope: scope.clone(),
                    }),
                );
                if request.sort == FeedSort::Top {
                    pairs.append_pair("t", request.top_range.api_value());
                }
                if scope.subreddit().is_some() {
                    pairs.append_pair("restrict_sr", "on");
                }
            }
            Ok(url)
        }
        spec => {
            let segment = match request.sort {
                FeedSort::Hot => "",
                FeedSort::New => "new",
                FeedSort::Top => "top",
                FeedSort::Rising => "rising",
                FeedSort::Controversial => "controversial",
                FeedSort::Best => {
                    return Err(ProviderError::Unsupported(
                        "the HTML listing has no best sort".into(),
                    ));
                }
            };
            let (mut path, trailing_slash) = match (flavor, spec) {
                // Redlib has no `/new` route for the front page. Its own
                // navigation links use `/r/popular/<sort>`, so use that route.
                (HtmlFlavor::Redlib, FeedSpec::Home | FeedSpec::Popular) => {
                    (String::from("/r/popular"), false)
                }
                (HtmlFlavor::Redlib, FeedSpec::Subreddit(name)) => {
                    let name = normalize_subreddit(name).ok_or_else(|| {
                        ProviderError::InvalidInput("invalid subreddit name".into())
                    })?;
                    (format!("/r/{name}"), false)
                }
                (HtmlFlavor::OldReddit, FeedSpec::Home) => (String::from("/"), true),
                (HtmlFlavor::OldReddit, FeedSpec::Popular) => (String::from("/r/popular/"), true),
                (HtmlFlavor::OldReddit, FeedSpec::Subreddit(name)) => {
                    let name = normalize_subreddit(name).ok_or_else(|| {
                        ProviderError::InvalidInput("invalid subreddit name".into())
                    })?;
                    (format!("/r/{name}/"), true)
                }
                (_, FeedSpec::Search { .. }) => unreachable!("handled above"),
            };
            if !segment.is_empty() {
                if !path.ends_with('/') {
                    path.push('/');
                }
                path.push_str(segment);
                if trailing_slash {
                    path.push('/');
                }
            } else if trailing_slash && !path.ends_with('/') {
                path.push('/');
            }
            let mut url = base.join(&path)?;
            if request.sort == FeedSort::Top {
                url.query_pairs_mut()
                    .append_pair("t", request.top_range.api_value());
            }
            Ok(url)
        }
    }
}

/// Per-provider caches populated from feed pages. Keeping post URLs lets the
/// detail/comment screens work without guessing URL patterns.
#[derive(Default)]
pub(crate) struct HtmlState {
    posts: Mutex<HashMap<String, Post>>,
    urls: Mutex<HashMap<String, String>>,
    subreddit: Mutex<Option<(String, Subreddit)>>,
}

impl HtmlState {
    pub(crate) fn remember_posts(&self, posts: &[Post]) {
        let mut cached = self.posts.lock().unwrap_or_else(|e| e.into_inner());
        let mut urls = self.urls.lock().unwrap_or_else(|e| e.into_inner());
        for post in posts {
            cached.insert(post.id.clone(), post.clone());
            if !post.permalink.is_empty() {
                urls.insert(post.id.clone(), post.permalink.clone());
            }
        }
        // Keep the cache bounded during long sessions.
        if cached.len() > 2_000 {
            cached.clear();
            for post in posts {
                cached.insert(post.id.clone(), post.clone());
            }
            if urls.len() > 2_000 {
                urls.clear();
                for post in posts {
                    if !post.permalink.is_empty() {
                        urls.insert(post.id.clone(), post.permalink.clone());
                    }
                }
            }
        }
    }

    pub(crate) fn cached_post(&self, id: &str) -> Option<Post> {
        self.posts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    pub(crate) fn post_url(&self, id: &str) -> Option<String> {
        self.urls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    pub(crate) fn remember_subreddit(&self, subreddit: Subreddit) {
        *self.subreddit.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((subreddit.name.to_ascii_lowercase(), subreddit));
    }

    pub(crate) fn cached_subreddit(&self, name: &str) -> Option<Subreddit> {
        let guard = self.subreddit.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .as_ref()
            .filter(|(cached, _)| cached.eq_ignore_ascii_case(name))
            .map(|(_, subreddit)| subreddit.clone())
    }
}

pub(crate) fn comments_url(
    state: &HtmlState,
    base: &Url,
    post_id: &str,
    sort: CommentSort,
    fallback: bool,
    flavor: HtmlFlavor,
) -> Result<Url, ProviderError> {
    let raw = if let Some(url) = state.post_url(post_id) {
        url
    } else if fallback {
        base.join(&format!("/comments/{post_id}/"))?.to_string()
    } else {
        return Err(ProviderError::NotFound(format!(
            "no HTML comment URL is known for post {post_id}; open it from a feed first"
        )));
    };
    let mut url = Url::parse(&raw)
        .map_err(|e| ProviderError::Parse(format!("invalid post permalink for {post_id}: {e}")))?;
    {
        let mut pairs = url.query_pairs_mut();
        match flavor {
            // Old Reddit's comment endpoint accepts the larger limit and the
            // documented sort values.
            HtmlFlavor::OldReddit => {
                pairs.append_pair("limit", "500");
                pairs.append_pair("sort", sort.api_value());
            }
            // Keep the canonical Redlib URL for the default sort. Adding
            // parameters can bypass the instance's cached page and trigger a
            // fresh Reddit fetch (and its rate limit). Only add a sort when the
            // user actually chose a non-default one.
            HtmlFlavor::Redlib => {
                if sort != CommentSort::Best {
                    pairs.append_pair("sort", sort.api_value());
                }
            }
        }
    }
    Ok(url)
}

pub(crate) fn policy_note(source: &str) -> String {
    format!(
        "{source} HTML mode is not Reddit's official Data API. It uses the public HTML page, does not authenticate, and is subject to the source's terms, robots policy and availability. If you use a third-party Redlib instance, that server can see your requests and IP address."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FeedRequest, FeedSort, FeedSpec, SearchScope, TopRange};

    #[test]
    fn redlib_uses_its_real_routes() {
        let base = Url::parse("https://redlib.example").unwrap();
        let home_new = FeedRequest::first(FeedSpec::Home, FeedSort::New, TopRange::All, 25);
        assert_eq!(
            listing_url(&base, &home_new, HtmlFlavor::Redlib)
                .unwrap()
                .as_str(),
            "https://redlib.example/r/popular/new"
        );
        let sub_top = FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::Top,
            TopRange::Week,
            25,
        );
        assert_eq!(
            listing_url(&base, &sub_top, HtmlFlavor::Redlib)
                .unwrap()
                .as_str(),
            "https://redlib.example/r/rust/top?t=week"
        );
        let search = FeedRequest::first(
            FeedSpec::Search {
                query: "async rust".into(),
                scope: SearchScope::All,
            },
            FeedSort::Best,
            TopRange::All,
            25,
        );
        let url = listing_url(&base, &search, HtmlFlavor::Redlib).unwrap();
        assert!(url.path().starts_with("/search"));
        assert!(url.query().unwrap_or("").contains("sort=relevance"));
    }

    #[test]
    fn old_reddit_keeps_its_trailing_slash_routes() {
        let base = Url::parse("https://old.reddit.com").unwrap();
        let home_new = FeedRequest::first(FeedSpec::Home, FeedSort::New, TopRange::All, 25);
        assert_eq!(
            listing_url(&base, &home_new, HtmlFlavor::OldReddit)
                .unwrap()
                .as_str(),
            "https://old.reddit.com/new/"
        );
        let sub_new = FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::New,
            TopRange::All,
            25,
        );
        assert_eq!(
            listing_url(&base, &sub_new, HtmlFlavor::OldReddit)
                .unwrap()
                .as_str(),
            "https://old.reddit.com/r/rust/new/"
        );
    }
}
