//! Integration tests for the HTML providers against a local mock server.
//!
//! No external network access is used, and nothing here claims that a live
//! Old Reddit or Redlib instance was reached.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use reddtty::error::ProviderError;
use reddtty::models::{
    CommentSort, FeedRequest, FeedSort, FeedSpec, PostContent, SearchScope, TopRange,
};
use reddtty::providers::ContentProvider;
use reddtty::providers::html::client::HtmlClientConfig;
use reddtty::providers::html::{OldRedditProvider, RedlibProvider};

const OLD_FEED: &str = include_str!("../fixtures/html/old_reddit_feed.html");
const OLD_COMMENTS: &str = include_str!("../fixtures/html/old_reddit_comments.html");
const REDLIB_FEED: &str = include_str!("../fixtures/html/redlib_feed.html");
const REDLIB_COMMENTS: &str = include_str!("../fixtures/html/redlib_comments.html");
const LOGIN: &str = include_str!("../fixtures/html/login_wall.html");
const BROKEN: &str = include_str!("../fixtures/html/structure_changed.html");

#[derive(Clone, Debug)]
struct Reply {
    status: u16,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn html(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    fn status(status: u16) -> Self {
        Self {
            status,
            content_type: "text/html",
            headers: Vec::new(),
            body: format!("<html><body>status {status}</body></html>").into_bytes(),
        }
    }

    fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

#[derive(Clone, Debug)]
struct Recorded {
    path: String,
    headers: Vec<(String, String)>,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

struct MockServer {
    base: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

fn spawn_server<F>(handler: F) -> MockServer
where
    F: Fn(&str, usize) -> Reply + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let counter = Arc::new(AtomicUsize::new(0));
    let handler = Arc::new(handler);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut raw = Vec::new();
            let mut buf = [0u8; 2048];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        raw.extend_from_slice(&buf[..n]);
                        if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let text = String::from_utf8_lossy(&raw);
            let head = text.split("\r\n\r\n").next().unwrap_or("");
            let mut headers = Vec::new();
            let mut lines = head.lines();
            let first = lines.next().unwrap_or("");
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("").to_string();
            for line in lines {
                if let Some((key, value)) = line.split_once(':') {
                    headers.push((key.trim().to_ascii_lowercase(), value.trim().to_string()));
                }
            }
            let index = counter.fetch_add(1, Ordering::SeqCst);
            recorded.lock().unwrap().push(Recorded {
                path: path.clone(),
                headers,
            });
            let reply = if method == "GET" {
                handler(&path, index)
            } else {
                Reply::status(405)
            };
            let head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                if reply.status == 200 { "OK" } else { "Error" },
                reply.content_type,
                reply.body.len()
            );
            let mut response = head.into_bytes();
            for (name, value) in &reply.headers {
                response.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
            }
            response.extend_from_slice(b"\r\n");
            response.extend_from_slice(&reply.body);
            let _ = stream.write_all(&response);
        }
    });
    MockServer { base, requests }
}

fn old_provider(server: &MockServer, allow_network: bool) -> OldRedditProvider {
    let mut config = HtmlClientConfig::new(&server.base, "reddtty/0.1.0 (tests)");
    config.allow_network = allow_network;
    OldRedditProvider::new(config).unwrap()
}

fn redlib_provider(server: &MockServer, allow_network: bool) -> RedlibProvider {
    let mut config = HtmlClientConfig::new(&server.base, "reddtty/0.1.0 (tests)");
    config.allow_network = allow_network;
    RedlibProvider::new(config).unwrap()
}

fn home(sort: FeedSort) -> FeedRequest {
    FeedRequest::first(FeedSpec::Home, sort, TopRange::Week, 25)
}

#[tokio::test]
async fn old_reddit_feed_pagination_and_user_agent() {
    let base_for_reply = Arc::new(Mutex::new(String::new()));
    let base_clone = base_for_reply.clone();
    let server = spawn_server(move |_path, _index| {
        let base = base_clone.lock().unwrap().clone();
        Reply::html(OLD_FEED.replace("https://old.reddit.com", &base))
    });
    *base_for_reply.lock().unwrap() = server.base.clone();
    let provider = old_provider(&server, true);

    let first = provider.get_feed(home(FeedSort::New)).await.unwrap();
    assert_eq!(first.posts.len(), 6);
    assert!(first.has_more);
    let after = first.after.expect("next-page URL");
    assert!(after.starts_with(&server.base));
    assert!(after.contains("after=t3_next"));
    assert!(matches!(first.posts[0].content, PostContent::Text));
    assert!(matches!(first.posts[1].content, PostContent::Image { .. }));
    assert!(matches!(
        first.posts[2].content,
        PostContent::Gallery { .. }
    ));
    assert!(matches!(first.posts[3].content, PostContent::Link { .. }));
    assert!(matches!(first.posts[4].content, PostContent::Video { .. }));
    assert!(first.posts[5].nsfw && first.posts[5].spoiler);

    let second = FeedRequest {
        after: Some(after),
        ..home(FeedSort::New)
    };
    provider.get_feed(second).await.unwrap();
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].path.contains("/new/"));
    assert!(requests[1].path.contains("after=t3_next"));
    let ua = requests[0].header("user-agent").unwrap_or("");
    assert!(ua.contains("reddtty"));
    assert_eq!(
        requests[0].header("accept").unwrap_or(""),
        "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1"
    );
    // A normal Old Reddit page contains a login form in the header; parsing it
    // must not be mistaken for a login wall.
}

#[tokio::test]
async fn old_reddit_sort_urls_and_search_params_are_real_changes() {
    let server = spawn_server(|_path, _index| Reply::html(OLD_FEED));
    let provider = old_provider(&server, true);
    for (sort, expected) in [
        (FeedSort::Hot, "/r/rust/"),
        (FeedSort::New, "/r/rust/new/"),
        (FeedSort::Rising, "/r/rust/rising/"),
        (FeedSort::Controversial, "/r/rust/controversial/"),
        (FeedSort::Top, "/r/rust/top/?t=week"),
    ] {
        let request =
            FeedRequest::first(FeedSpec::Subreddit("rust".into()), sort, TopRange::Week, 25);
        provider.get_feed(request).await.unwrap();
        let paths: Vec<String> = server
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.path.clone())
            .collect();
        assert!(
            paths.last().unwrap().contains(expected),
            "{sort:?}: expected {expected}, got {paths:?}"
        );
    }

    let search = FeedRequest::first(
        FeedSpec::Search {
            query: "terminal rust".into(),
            scope: SearchScope::All,
        },
        FeedSort::Best,
        TopRange::Week,
        25,
    );
    provider.get_feed(search).await.unwrap();
    let path = server.requests.lock().unwrap().last().unwrap().path.clone();
    assert!(path.starts_with("/search?"));
    assert!(path.contains("q=terminal+rust"));
    assert!(path.contains("sort=relevance"));
}

#[tokio::test]
async fn sorting_fetches_a_different_page_not_just_a_different_label() {
    let server = spawn_server(|path, _index| {
        if path.contains("/new/") {
            Reply::html(OLD_FEED.replace("Text post title", "New-sort first post"))
        } else {
            Reply::html(OLD_FEED)
        }
    });
    let provider = old_provider(&server, true);
    let hot = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::Hot,
            TopRange::All,
            25,
        ))
        .await
        .unwrap();
    let new = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::New,
            TopRange::All,
            25,
        ))
        .await
        .unwrap();
    assert_eq!(hot.posts[0].title, "Text post title");
    assert_eq!(new.posts[0].title, "New-sort first post");
}

#[tokio::test]
async fn old_reddit_comments_are_nested_and_deleted_comments_are_kept() {
    let base_for_reply = Arc::new(Mutex::new(String::new()));
    let base_clone = base_for_reply.clone();
    let server = spawn_server(move |path, _index| {
        let base = base_clone.lock().unwrap().clone();
        if path.contains("/comments/") {
            Reply::html(OLD_COMMENTS.replace("https://old.reddit.com", &base))
        } else {
            Reply::html(OLD_FEED.replace("https://old.reddit.com", &base))
        }
    });
    *base_for_reply.lock().unwrap() = server.base.clone();
    let provider = old_provider(&server, true);
    let feed = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Subreddit("rust".into()),
            FeedSort::Hot,
            TopRange::All,
            25,
        ))
        .await
        .unwrap();
    assert_eq!(feed.posts[0].id, "aaa");
    let comments = provider
        .get_comments("aaa", CommentSort::Top)
        .await
        .unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].replies[0].id, "c2");
    assert_eq!(comments[0].replies[0].replies[0].id, "c3");
    assert!(comments[1].deleted);
    let requests = server.requests.lock().unwrap();
    let comment_request = requests
        .iter()
        .find(|r| r.path.contains("/comments/aaa/"))
        .expect("comment request");
    assert!(comment_request.path.contains("limit=500"));
    assert!(comment_request.path.contains("sort=top"));
}

#[tokio::test]
async fn redlib_feed_and_comments_use_its_own_parser() {
    let base_for_reply = Arc::new(Mutex::new(String::new()));
    let base_clone = base_for_reply.clone();
    let server = spawn_server(move |path, _index| {
        let base = base_clone.lock().unwrap().clone();
        if path.contains("/comments/") {
            Reply::html(REDLIB_COMMENTS.replace("https://redlib.example", &base))
        } else {
            Reply::html(REDLIB_FEED.replace("https://redlib.example", &base))
        }
    });
    *base_for_reply.lock().unwrap() = server.base.clone();
    let provider = redlib_provider(&server, true);
    let first_request = FeedRequest::first(
        FeedSpec::Subreddit("rust".into()),
        FeedSort::Hot,
        TopRange::All,
        25,
    );
    let feed = provider.get_feed(first_request.clone()).await.unwrap();
    assert_eq!(feed.posts.len(), 6);
    assert!(matches!(feed.posts[1].content, PostContent::Image { .. }));
    assert!(matches!(feed.posts[2].content, PostContent::Gallery { .. }));
    let after = feed.after.clone().expect("Redlib footer next link");
    assert!(after.contains("after=t3_next"));
    provider
        .get_feed(FeedRequest {
            after: Some(after),
            ..first_request
        })
        .await
        .unwrap();
    {
        let requests = server.requests.lock().unwrap();
        let pagination = requests
            .iter()
            .find(|r| r.path.contains("after=t3_next"))
            .expect("pagination request");
        assert!(pagination.path.contains("sort=hot"), "{}", pagination.path);
    }
    let comments = provider
        .get_comments("aaa", CommentSort::Best)
        .await
        .unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].replies[0].id, "c2");
    assert!(comments[1].deleted);
    {
        let requests = server.requests.lock().unwrap();
        let request = requests
            .iter()
            .find(|r| r.path.contains("/comments/aaa/"))
            .expect("comment request");
        assert!(
            !request.path.contains("limit="),
            "Redlib must use the canonical URL: {}",
            request.path
        );
        assert!(
            !request.path.contains("sort="),
            "default Best sort must not add a query: {}",
            request.path
        );
    }

    provider
        .get_comments("aaa", CommentSort::New)
        .await
        .unwrap();
    let requests = server.requests.lock().unwrap();
    let request = requests
        .iter()
        .rev()
        .find(|r| r.path.contains("/comments/aaa/"))
        .expect("comment request");
    assert!(request.path.contains("sort=new"), "{}", request.path);
    assert!(!request.path.contains("limit="), "{}", request.path);
}

#[tokio::test]
async fn html_network_is_gated_before_any_request() {
    let server = spawn_server(|_path, _index| Reply::html(OLD_FEED));
    let provider = old_provider(&server, false);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(matches!(error, ProviderError::Disabled(_)), "{error:?}");
    assert!(server.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unauthorized_forbidden_and_rate_limited_responses_are_explicit() {
    let unauthorized = spawn_server(|_path, _index| Reply::status(401));
    let provider = old_provider(&unauthorized, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(
        matches!(error, ProviderError::LoginRequired(_)),
        "{error:?}"
    );

    let forbidden = spawn_server(|_path, _index| Reply::status(403));
    let provider = old_provider(&forbidden, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(matches!(error, ProviderError::Forbidden(_)), "{error:?}");

    let limited = spawn_server(|_path, _index| Reply::status(429).header("Retry-After", "17"));
    let provider = old_provider(&limited, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(
        matches!(
            error,
            ProviderError::RateLimited {
                retry_after: Some(17)
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn login_and_changed_html_are_not_empty_feeds() {
    let login = spawn_server(|_path, _index| Reply::html(LOGIN));
    let provider = old_provider(&login, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(
        matches!(error, ProviderError::LoginRequired(_)),
        "{error:?}"
    );

    let changed = spawn_server(|_path, _index| Reply::html(BROKEN));
    let provider = old_provider(&changed, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(
        matches!(error, ProviderError::HtmlStructureChanged(_)),
        "{error:?}"
    );
}

#[tokio::test]
async fn redlib_block_error_page_is_clean_access_denied() {
    let server = spawn_server(|_path, _index| {
        Reply {
        status: 404,
        content_type: "text/html; charset=utf-8",
        headers: Vec::new(),
        body: b"<!doctype html><html><head><title>Error: Reddit blocked this request (403 Forbidden)</title></head><body><main><h1>Error: Reddit blocked this request (403 Forbidden)</h1></main></body></html>".to_vec(),
    }
    });
    let provider = redlib_provider(&server, true);
    let error = provider.get_feed(home(FeedSort::New)).await.unwrap_err();
    let text = error.to_string();
    assert!(matches!(error, ProviderError::Forbidden(_)), "{error:?}");
    assert!(text.contains("access block"), "{text}");
    assert!(
        !text.contains("<!doctype"),
        "raw HTML leaked into the error: {text}"
    );
}

#[tokio::test]
async fn unexpected_content_type_is_rejected() {
    let server = spawn_server(|_path, _index| Reply::json("{\"not\":\"html\"}"));
    let provider = old_provider(&server, true);
    let error = provider.get_feed(home(FeedSort::Hot)).await.unwrap_err();
    assert!(matches!(error, ProviderError::Parse(_)), "{error:?}");
    assert!(error.to_string().contains("content-type"));
}

#[tokio::test]
async fn invalid_pagination_url_is_rejected_without_a_request() {
    let server = spawn_server(|_path, _index| Reply::html(OLD_FEED));
    let provider = old_provider(&server, true);
    let request = FeedRequest {
        after: Some("not a url".into()),
        ..home(FeedSort::Hot)
    };
    let error = provider.get_feed(request).await.unwrap_err();
    assert!(matches!(error, ProviderError::InvalidInput(_)), "{error:?}");
    assert!(server.requests.lock().unwrap().is_empty());
}
