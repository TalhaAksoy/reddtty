//! App state and input unit tests.

use super::*;
use crate::models::PostContent;

impl App {
    // ------------------------------------------------------------------
    pub fn test_with_posts(posts: Vec<Post>) -> Self {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let settings = Settings::default();
        let provider: Arc<dyn ContentProvider> = Arc::new(TestProvider);
        let images = ImageManager::disabled(tx.clone());
        let id = ViewId(1);
        let feed = FeedView::with_posts(id, posts);
        let mut app = Self {
            provider,
            settings,
            config_path: std::env::temp_dir().join("reddtty-test-config.toml"),
            tx,
            nav: vec![View::Feed(feed)],
            focus: Focus::Feed,
            sidebar: SidebarState::default(),
            modal: None,
            notifications: VecDeque::new(),
            images,
            spinner: 0,
            should_quit: false,
            size: Rect::new(0, 0, 100, 30),
            hit_map: HitMap::default(),
            force_clear: false,
            last_click: None,
            revealed: HashSet::new(),
            next_view_id: 2,
            next_request_id: 1,
            dirty: true,
            demo: true,
            provider_label: "DEMO",
            provider_status: None,
            persist_error_shown: true,
            persist_theme: Settings::default().general.theme,
            persist_images_enabled: false,
            persist_images_protocol: crate::config::ImageProtocolSetting::Disabled,
        };
        app.set_size(100, 30);
        app
    }
}

struct TestProvider;

#[async_trait::async_trait]
impl ContentProvider for TestProvider {
    fn name(&self) -> &'static str {
        "test"
    }
    fn is_demo(&self) -> bool {
        true
    }
    fn capabilities(&self) -> crate::providers::ProviderCapabilities {
        crate::providers::ProviderCapabilities {
            search: true,
            demo: true,
            ..Default::default()
        }
    }
    async fn get_feed(&self, _request: FeedRequest) -> Result<FeedPage, ProviderError> {
        Ok(FeedPage::default())
    }
    async fn get_post(&self, _post_id: &str) -> Result<Post, ProviderError> {
        Err(ProviderError::NotFound("test".into()))
    }
    async fn get_comments(
        &self,
        _post_id: &str,
        _sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        Ok(Vec::new())
    }
    async fn get_subreddit(&self, _name: &str) -> Result<Subreddit, ProviderError> {
        Err(ProviderError::NotFound("test".into()))
    }
}

fn post(id: &str) -> Post {
    Post {
        id: id.into(),
        subreddit: "rust".into(),
        author: "alice".into(),
        title: format!("Post {id}"),
        body: "body ".repeat(50),
        created_utc: 0,
        score: 10,
        num_comments: 3,
        permalink: "/r/rust/comments/x".into(),
        content: PostContent::Text,
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
fn back_preserves_feed_selection() {
    let posts = (0..20).map(|i| post(&format!("p{i}"))).collect();
    let mut app = App::test_with_posts(posts);
    app.prepare();
    app.focus = Focus::Feed;
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    let selected = app.current_feed().unwrap().selected;
    assert_eq!(selected, 2);
    let post = app.current_feed().unwrap().posts[selected].clone();
    let id = app.alloc_view();
    app.nav.push(View::Post(PostView::with_comments(
        id,
        post,
        Vec::new(),
        CommentSort::Best,
    )));
    app.focus = Focus::Comments;
    app.back();
    assert!(matches!(app.current(), Some(View::Feed(_))));
    assert_eq!(app.current_feed().unwrap().selected, 2);
}

#[test]
fn sort_modal_contains_supported_sorts() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_sort_modal();
    let Some(Modal::Sort(modal)) = &app.modal else {
        panic!("expected sort modal");
    };
    assert_eq!(modal.len(), FeedSort::ALL.len() + 1);
}

#[test]
fn top_range_modal_opens() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Range(_))));
}

#[test]
fn text_modal_captures_q_and_j() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_goto_modal();
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    let Some(Modal::Text(modal)) = &app.modal else {
        panic!("expected text modal");
    };
    assert_eq!(modal.value, "rust");
    assert!(!app.should_quit);
}

#[test]
fn escape_closes_modal_not_screen() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_help();
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.modal.is_none());
    assert!(matches!(app.current(), Some(View::Feed(_))));
}

#[test]
fn tab_cycles_feed_focus() {
    let mut app = App::test_with_posts(vec![post("p")]);
    assert_eq!(app.focus, Focus::Feed);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Sidebar);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Feed);
}

#[test]
fn flip_theme_updates_setting() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let before = app.settings.general.theme;
    app.toggle_theme();
    assert_ne!(before, app.settings.general.theme);
}

#[test]
fn reveal_marks_post() {
    let mut p = post("p");
    p.nsfw = true;
    let mut app = App::test_with_posts(vec![p]);
    app.reveal_current();
    assert!(app.revealed.contains("p"));
    assert!(!app.is_hidden(app.current_feed().unwrap().posts.first().unwrap()));
}

#[test]
fn html_sort_error_is_actionable_and_does_not_fall_back() {
    let message = describe_feed_error(
        "REDLIB",
        &FeedSpec::Home,
        FeedSort::New,
        &ProviderError::Forbidden("blocked".into()),
    );
    assert!(
        message.contains("New is not available for Home from REDLIB"),
        "{message}"
    );
    assert!(message.contains("Hot may still work"), "{message}");
    assert!(
        message.contains("does not bypass access controls"),
        "{message}"
    );
    assert!(message.contains("access denied or CAPTCHA"), "{message}");

    let demo = describe_feed_error(
        "DEMO",
        &FeedSpec::Home,
        FeedSort::New,
        &ProviderError::Forbidden("blocked".into()),
    );
    assert_eq!(demo, "Reddit refused this request: blocked");
}

#[test]
fn removing_sidebar_favorite_requires_confirmation() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.settings.favorites.push("rust".into());
    app.focus = Focus::Sidebar;
    let index = app
        .sidebar_entries()
        .iter()
        .position(|entry| entry.label == "r/rust" && entry.favorite)
        .expect("default rust favorite");
    app.sidebar.selected = index;
    assert!(app.settings.is_favorite("rust"));

    app.toggle_favorite();
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    assert!(
        app.settings.is_favorite("rust"),
        "must not remove before confirmation"
    );

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.modal.is_none());
    assert!(
        app.settings.is_favorite("rust"),
        "cancel must keep the favorite"
    );

    app.toggle_favorite();
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert!(app.modal.is_none());
    assert!(
        !app.settings.is_favorite("rust"),
        "confirmation removes the favorite"
    );
}

#[test]
fn stale_requests_are_ignored() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let id = app.current_id().unwrap();
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 999,
        append: false,
        result: Ok(FeedPage {
            posts: vec![post("other")],
            after: None,
            has_more: false,
        }),
    });
    assert_eq!(app.current_feed().unwrap().posts[0].id, "p");
    // The provider is fixed for the session; a stale result cannot relabel the
    // UI as if it came from another source.
    assert_eq!(app.provider_label, "DEMO");
}

#[test]
fn api_errors_are_shown_in_the_feed() {
    for (error, expected) in [
        (
            ProviderError::AuthRequired("token expired".into()),
            "authentication is required",
        ),
        (
            ProviderError::Forbidden("no".into()),
            "Reddit refused this request",
        ),
        (ProviderError::NotFound("gone".into()), "Content not found"),
        (
            ProviderError::RateLimited {
                retry_after: Some(5),
            },
            "Rate limited",
        ),
        (
            ProviderError::Server(500, "down".into()),
            "Reddit server error",
        ),
        (ProviderError::Timeout(20), "Request timed out"),
        (ProviderError::Network("offline".into()), "Network error"),
        (ProviderError::Parse("bad".into()), "Could not parse"),
    ] {
        let mut app = App::test_with_posts(Vec::new());
        let id = app.current_id().unwrap();
        app.current_feed_mut().unwrap().request = Some(11);
        app.handle_event(AppEvent::FeedLoaded {
            view: id,
            request: 11,
            append: false,
            result: Err(error),
        });
        let text = app.current_feed().unwrap().error.clone().unwrap();
        assert!(text.contains(expected), "expected `{expected}` in `{text}`");
    }
}

#[test]
fn append_error_stops_pagination_storm() {
    let mut app = App::test_with_posts((0..3).map(|i| post(&format!("p{i}"))).collect());
    let id = app.current_id().unwrap();
    {
        let feed = app.current_feed_mut().unwrap();
        feed.has_more = true;
        feed.after = Some("d:3".into());
        feed.more_request = Some(22);
    }
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 22,
        append: true,
        result: Err(ProviderError::Server(500, "down".into())),
    });
    let feed = app.current_feed().unwrap();
    assert!(!feed.loading_more);
    assert!(!feed.has_more);
    assert!(feed.more_request.is_none());
}

#[test]
fn errors_are_mapped_to_feed_error_when_empty() {
    let mut app = App::test_with_posts(Vec::new());
    let id = app.current_id().unwrap();
    app.current_feed_mut().unwrap().request = Some(7);
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 7,
        append: false,
        result: Err(ProviderError::Network("offline".into())),
    });
    assert!(app.current_feed().unwrap().error.is_some());
}
