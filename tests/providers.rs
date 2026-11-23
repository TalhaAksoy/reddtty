//! Integration tests for the live Reddit provider and image downloader using a
//! local HTTP mock server. No external network access is used.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use reddtty::error::ProviderError;
use reddtty::models::{CommentSort, FeedRequest, FeedSort, FeedSpec, PostContent, TopRange};
use reddtty::providers::ContentProvider;
use reddtty::providers::reddit::{RedditConfig, RedditProvider};

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type", "application/json".into())],
            body: body.into().into_bytes(),
        }
    }

    fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec![("content-type", "application/json".into())],
            body: format!("{{\"error\":\"status {status}\"}}").into_bytes(),
        }
    }

    fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type", "image/png".into())],
            body,
        }
    }
}

#[derive(Clone, Debug)]
struct RecordedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl RecordedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

struct MockServer {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    full_requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

fn spawn_server<F>(handler: F) -> MockServer
where
    F: Fn(&str, &str, usize) -> Reply + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let full_requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let recorded_full = full_requests.clone();
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
            let header_end = raw
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|pos| pos + 4);
            if let Some(header_end) = header_end {
                let header_text = String::from_utf8_lossy(&raw[..header_end]);
                let content_length = header_text
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        if !key.eq_ignore_ascii_case("content-length") {
                            return None;
                        }
                        value.trim().parse::<usize>().ok()
                    })
                    .unwrap_or(0);
                while raw.len() < header_end + content_length {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                }
            }

            let text = String::from_utf8_lossy(&raw);
            let (head, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_ref(), ""));
            let mut headers = Vec::new();
            for line in head.lines().skip(1) {
                if let Some((key, value)) = line.split_once(':') {
                    headers.push((key.trim().to_ascii_lowercase(), value.trim().to_string()));
                }
            }
            let first = head.lines().next().unwrap_or("");
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");
            let index = counter.fetch_add(1, Ordering::SeqCst);
            recorded.lock().unwrap().push(format!("{method} {path}"));
            recorded_full.lock().unwrap().push(RecordedRequest {
                method: method.to_string(),
                path: path.to_string(),
                headers,
                body: body.to_string(),
            });
            let reply = handler(method, path, index);
            let reason = match reply.status {
                200 => "OK",
                401 => "Unauthorized",
                403 => "Forbidden",
                404 => "Not Found",
                429 => "Too Many Requests",
                500 => "Internal Server Error",
                _ => "Error",
            };
            let mut head = format!(
                "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                reason,
                reply.body.len()
            );
            for (key, value) in &reply.headers {
                head.push_str(&format!("{key}: {value}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&reply.body);
            let _ = stream.flush();
        }
    });
    MockServer {
        base,
        requests,
        full_requests,
    }
}

fn make_provider(server: &MockServer) -> RedditProvider {
    RedditProvider::new(
        RedditConfig::new("client-id", "client-secret", "reddtty/0.1.0 (tests)")
            .with_endpoints(&server.base, format!("{}/token", server.base)),
    )
    .unwrap()
}

fn token_reply() -> Reply {
    Reply::json(r#"{"access_token":"test-access-token","expires_in":3600,"token_type":"bearer"}"#)
}

fn listing(after: Option<&str>) -> String {
    let after = after
        .map(|a| format!("\"{a}\""))
        .unwrap_or_else(|| "null".into());
    format!(
        r#"{{"kind":"Listing","data":{{"after":{after},"children":[
            {{"kind":"t3","data":{{
                "id":"abc123","subreddit":"rust","author":"alice","title":"A test post",
                "selftext":"hello **world**","created_utc":1700000000.0,"score":42,
                "num_comments":7,"permalink":"/r/rust/comments/abc123/a_test_post/",
                "is_self":true,"over_18":false,"spoiler":false,"link_flair_text":"Discussion"
            }}}}
        ]}}}}"#
    )
}

fn comments_reply() -> String {
    r#"[
        {"kind":"Listing","data":{"children":[{"kind":"t3","data":{
            "id":"abc123","subreddit":"rust","author":"alice","title":"A test post",
            "selftext":"hello","created_utc":1700000000.0,"score":42,"num_comments":1,
            "permalink":"/r/rust/comments/abc123/a_test_post/","is_self":true
        }}]}},
        {"kind":"Listing","data":{"children":[
            {"kind":"t1","data":{"id":"c1","parent_id":"t3_abc123","author":"bob","body":"root",
                "score":5,"created_utc":1700000100.0,"replies":{"kind":"Listing","data":{"children":[
                    {"kind":"t1","data":{"id":"c2","parent_id":"t1_c1","author":"carol","body":"child","score":2,"created_utc":1700000200.0}}
                ]}}}},
            {"kind":"more","data":{"id":"m1","parent_id":"t3_abc123","count":3,"children":["x","y","z"]}}
        ]}}
    ]"#
    .to_string()
}

#[tokio::test]
async fn feed_success_and_cursor_pagination() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        if path.starts_with("/best") {
            if path.contains("after=t3_after") {
                return Reply::json(listing(None));
            }
            return Reply::json(listing(Some("t3_after")));
        }
        Reply::status(404)
    });
    let provider = make_provider(&server);
    let first = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Best,
            TopRange::All,
            25,
        ))
        .await
        .unwrap();
    assert_eq!(first.posts.len(), 1);
    let post = &first.posts[0];
    assert_eq!(post.title, "A test post");
    assert_eq!(post.content, PostContent::Text);
    assert_eq!(post.flair.as_deref(), Some("Discussion"));
    assert!(first.has_more);
    assert_eq!(first.after.as_deref(), Some("t3_after"));

    let second = provider
        .get_feed(FeedRequest {
            spec: FeedSpec::Home,
            sort: FeedSort::Best,
            top_range: TopRange::All,
            after: first.after,
            limit: 25,
        })
        .await
        .unwrap();
    assert_eq!(second.posts.len(), 1);
    assert!(!second.has_more);

    let requests = server.requests.lock().unwrap();
    assert!(requests.iter().any(|r| r.contains("raw_json=1")));
    assert!(requests.iter().any(|r| r.contains("after=t3_after")));
}

#[tokio::test]
async fn maps_http_errors_to_provider_errors() {
    for (status, check) in [(401u16, 0u8), (403, 1), (404, 2), (429, 3), (500, 4)] {
        let server = spawn_server(move |method, path, _| {
            if method == "POST" && path.starts_with("/token") {
                return token_reply();
            }
            let mut reply = Reply::status(status);
            if status == 429 {
                reply.headers.push(("retry-after", "7".into()));
            }
            reply
        });
        let provider = make_provider(&server);
        let error = provider
            .get_feed(FeedRequest::first(
                FeedSpec::Home,
                FeedSort::Hot,
                TopRange::All,
                25,
            ))
            .await
            .unwrap_err();
        match check {
            0 => assert!(matches!(error, ProviderError::AuthRequired(_)), "{error:?}"),
            1 => assert!(matches!(error, ProviderError::Forbidden(_)), "{error:?}"),
            2 => assert!(matches!(error, ProviderError::NotFound(_)), "{error:?}"),
            3 => assert!(
                matches!(
                    error,
                    ProviderError::RateLimited {
                        retry_after: Some(7)
                    }
                ),
                "{error:?}"
            ),
            4 => assert!(matches!(error, ProviderError::Server(500, _)), "{error:?}"),
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn invalid_json_and_missing_token_errors() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        Reply::json("this is not json")
    });
    let provider = make_provider(&server);
    let error = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::New,
            TopRange::All,
            10,
        ))
        .await
        .unwrap_err();
    assert!(matches!(error, ProviderError::Parse(_)), "{error:?}");

    let bad_token = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return Reply::status(401);
        }
        Reply::json(listing(None))
    });
    let provider = make_provider(&bad_token);
    let error = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::New,
            TopRange::All,
            10,
        ))
        .await
        .unwrap_err();
    assert!(matches!(error, ProviderError::AuthRequired(_)), "{error:?}");
}

#[tokio::test]
async fn unauthorized_feed_refreshes_token_once() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen2 = seen.clone();
    let server = spawn_server(move |method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        seen2.lock().unwrap().push(path.to_string());
        if seen2.lock().unwrap().len() == 1 {
            Reply::status(401)
        } else {
            Reply::json(listing(None))
        }
    });
    let provider = make_provider(&server);
    let page = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Hot,
            TopRange::All,
            10,
        ))
        .await
        .unwrap();
    assert_eq!(page.posts.len(), 1);
    let token_requests = server
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.contains("/token"))
        .count();
    assert_eq!(token_requests, 2, "expired token should be refreshed once");
}

#[tokio::test]
async fn timeout_is_reported() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        thread::sleep(Duration::from_millis(700));
        Reply::json(listing(None))
    });
    let mut config = RedditConfig::new("client-id", "client-secret", "reddtty/0.1.0 (tests)")
        .with_endpoints(&server.base, format!("{}/token", server.base));
    config.request_timeout = Duration::from_millis(150);
    let provider = RedditProvider::new(config).unwrap();
    let error = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Hot,
            TopRange::All,
            10,
        ))
        .await
        .unwrap_err();
    assert!(matches!(error, ProviderError::Timeout(_)), "{error:?}");
}

#[tokio::test]
async fn comments_and_morechildren_are_mapped() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        if path.starts_with("/comments/abc123") {
            return Reply::json(comments_reply());
        }
        if path.starts_with("/api/morechildren") {
            return Reply::json(
                r#"{"json":{"errors":[],"data":{"things":[
                    {"kind":"t1","data":{"id":"x","parent_id":"t1_c1","author":"xavier","body":"loaded","score":3,"created_utc":1700000300.0}},
                    {"kind":"t1","data":{"id":"x2","parent_id":"t1_x","author":"yara","body":"nested","score":1,"created_utc":1700000400.0}}
                ]}}}"#,
            );
        }
        if path.starts_with("/r/rust/about") {
            return Reply::json(
                r#"{"kind":"t5","data":{"display_name":"rust","title":"The Rust Programming Language","public_description":"A place for Rust.","subscribers":412000,"active_user_count":8400,"over18":false,"created_utc":1500000000.0}}"#,
            );
        }
        Reply::status(404)
    });
    let provider = make_provider(&server);

    let comments = provider
        .get_comments("abc123", CommentSort::Best)
        .await
        .unwrap();
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].replies.len(), 1);
    assert!(comments[1].is_more);
    assert_eq!(comments[1].more_children.len(), 3);

    let loaded = provider
        .get_more_comments("abc123", &["x".into(), "x2".into()], CommentSort::New)
        .await
        .unwrap();
    assert_eq!(loaded.len(), 1, "x2 should be nested under x");
    assert_eq!(loaded[0].id, "x");
    assert_eq!(loaded[0].replies.len(), 1);

    let subreddit = provider.get_subreddit("rust").await.unwrap();
    assert_eq!(subreddit.name, "rust");
    assert_eq!(subreddit.subscribers, 412_000);
    assert_eq!(subreddit.active_users, 8_400);

    let post = provider.get_post("abc123").await.unwrap();
    assert_eq!(post.title, "A test post");
}

#[tokio::test]
async fn search_uses_documented_parameters() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        if path.starts_with("/search") || path.contains("/search") {
            return Reply::json(listing(None));
        }
        Reply::status(404)
    });
    let provider = make_provider(&server);
    let page = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Search {
                query: "terminal graphics".into(),
                scope: reddtty::models::SearchScope::Subreddit("rust".into()),
            },
            FeedSort::Top,
            TopRange::Month,
            25,
        ))
        .await
        .unwrap();
    assert_eq!(page.posts.len(), 1);
    let requests = server.requests.lock().unwrap();
    let search = requests
        .iter()
        .find(|r| r.contains("/search"))
        .expect("search request");
    assert!(search.contains("q=terminal+graphics") || search.contains("q=terminal%20graphics"));
    assert!(search.contains("restrict_sr=1"));
    assert!(search.contains("type=link"));
    assert!(search.contains("sort=top"));
    assert!(search.contains("t=month"));
}

#[tokio::test]
async fn image_downloader_uses_local_http_server() {
    use reddtty::media::downloader::download_image;
    let png = make_png();
    let png2 = png.clone();
    let server = spawn_server(move |_method, path, _| {
        if path.starts_with("/ok.png") {
            Reply::bytes(png2.clone())
        } else if path.starts_with("/big.png") {
            Reply::bytes(vec![0u8; 4096])
        } else {
            Reply::status(404)
        }
    });
    let client = reqwest::Client::new();
    let bytes = download_image(
        &client,
        &format!("{}/ok.png", server.base),
        10_000,
        None,
        true,
    )
    .await
    .unwrap();
    assert_eq!(bytes.len(), png.len());
    let error = download_image(
        &client,
        &format!("{}/big.png", server.base),
        1024,
        None,
        true,
    )
    .await
    .unwrap_err();
    assert!(error.contains("too large") || error.contains("limit"));
    let error = download_image(
        &client,
        &format!("{}/missing.png", server.base),
        1024,
        None,
        true,
    )
    .await
    .unwrap_err();
    assert!(error.contains("404"));
}

fn make_png() -> Vec<u8> {
    let img = image::RgbImage::from_fn(8, 8, |x, y| image::Rgb([x as u8 * 8, y as u8 * 8, 80]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[tokio::test]
async fn rate_limit_cooldown_prevents_request_storm() {
    let feed_requests = Arc::new(AtomicUsize::new(0));
    let counter = feed_requests.clone();
    let server = spawn_server(move |method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        counter.fetch_add(1, Ordering::SeqCst);
        let mut reply = Reply::status(429);
        reply.headers.push(("retry-after", "60".into()));
        reply
    });
    let provider = make_provider(&server);
    let request = || FeedRequest::first(FeedSpec::Home, FeedSort::Hot, TopRange::All, 10);
    let first = provider.get_feed(request()).await.unwrap_err();
    assert!(matches!(first, ProviderError::RateLimited { .. }));
    let second = provider.get_feed(request()).await.unwrap_err();
    assert!(matches!(second, ProviderError::RateLimited { .. }));
    assert_eq!(
        feed_requests.load(Ordering::SeqCst),
        1,
        "the cooldown should prevent a second HTTP request"
    );
}

#[test]
fn provider_configuration_is_validated() {
    assert!(matches!(
        RedditProvider::new(RedditConfig::new("", "", "reddtty/test")),
        Err(ProviderError::AuthRequired(_))
    ));
    assert!(matches!(
        RedditProvider::new(RedditConfig::new("id", "secret", "not-a-client")),
        Err(ProviderError::InvalidInput(_))
    ));
}

fn token_config(server: &MockServer, refresh: Option<&str>) -> RedditConfig {
    let mut config = RedditConfig::new("client-id", "client-secret", "reddtty/0.1.0 (tests)")
        .with_endpoints(&server.base, format!("{}/token", server.base));
    config.refresh_token = refresh.map(str::to_string);
    config
}

#[tokio::test]
async fn oauth_request_uses_documented_form_headers_and_bearer_token() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        if path.starts_with("/best") {
            return Reply::json(listing(None));
        }
        Reply::status(404)
    });
    let provider = make_provider(&server);
    provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Best,
            TopRange::All,
            10,
        ))
        .await
        .unwrap();

    let requests = server.full_requests.lock().unwrap();
    let token = requests
        .iter()
        .find(|r| r.method == "POST" && r.path.starts_with("/token"))
        .expect("token request");
    assert_eq!(
        token.header("authorization"),
        Some("Basic Y2xpZW50LWlkOmNsaWVudC1zZWNyZXQ=")
    );
    assert!(token.header("user-agent").unwrap_or("").contains("reddtty"));
    assert!(token.body.contains("grant_type=client_credentials"));
    assert!(token.body.contains("scope=read"));

    let feed = requests
        .iter()
        .find(|r| r.method == "GET")
        .expect("feed request");
    assert_eq!(
        feed.header("authorization"),
        Some("Bearer test-access-token")
    );
    assert_eq!(feed.header("accept"), Some("application/json"));
    assert!(feed.path.contains("raw_json=1"));
}

#[tokio::test]
async fn refresh_token_grant_is_sent_without_scope() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return token_reply();
        }
        if path.starts_with("/hot") {
            return Reply::json(listing(None));
        }
        Reply::status(404)
    });
    let provider = RedditProvider::new(token_config(&server, Some("refresh-value"))).unwrap();
    provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Hot,
            TopRange::All,
            10,
        ))
        .await
        .unwrap();
    let requests = server.full_requests.lock().unwrap();
    let token = requests
        .iter()
        .find(|r| r.method == "POST")
        .expect("token request");
    assert!(token.body.contains("grant_type=refresh_token"));
    assert!(token.body.contains("refresh_token=refresh-value"));
    assert!(!token.body.contains("scope="));
}

#[tokio::test]
async fn token_response_without_access_token_is_a_parse_error() {
    let server = spawn_server(|method, path, _| {
        if method == "POST" && path.starts_with("/token") {
            return Reply::json(r#"{"expires_in":3600,"token_type":"bearer"}"#);
        }
        Reply::json(listing(None))
    });
    let provider = make_provider(&server);
    let error = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Hot,
            TopRange::All,
            10,
        ))
        .await
        .unwrap_err();
    assert!(matches!(error, ProviderError::Parse(_)), "{error:?}");
}
