//! App state and input unit tests.

use super::*;
use crate::models::PostContent;

impl App {
    // ------------------------------------------------------------------
    pub fn test_with_posts(posts: Vec<Post>) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let settings = Settings::default();
        let provider: Arc<dyn ContentProvider> = Arc::new(TestProvider);
        let images = ImageManager::disabled(tx.clone());
        let id = ViewId(1);
        let feed = FeedView::with_posts(id, posts);
        let mut app = Self {
            provider,
            settings,
            config_path: std::env::temp_dir().join(format!(
                "reddtty-test-config-{}-{}.toml",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            )),
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
            provider_label: "DEMO".into(),
            provider_status: None,
            persist_error_shown: true,
            no_color: false,
            config_corrupt: false,
            config_recovery: None,
            settings_cursor: 0,
            skip_exit_persist: false,
            persist_theme: Settings::default().general.theme,
            persist_images_enabled: false,
            persist_images_protocol: crate::config::ImageProtocolSetting::Disabled,
            persist_provider: Settings::default().provider,
            library: crate::library::Library::default(),
            library_path: std::env::temp_dir().join(format!(
                "reddtty-test-library-{}-{}.toml",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            )),
            library_corrupt: false,
            library_recovery: None,
            return_to_bookmarks: None,
            bookmark_delete_return: None,
            custom_redlib_return: None,
            health: SourceHealth::default(),
            request_starts: std::collections::HashMap::new(),
            recent_commands: Vec::new(),
            last_filter_undo: None,
            probe_request: None,
            pending_switch: None,
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
fn settings_config_picker_installs_preset_and_backs_up() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-settings-picker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "favorites = [\"linux\"]\n").unwrap();
    app.config_path = path.clone();
    app.settings.favorites = vec!["rust".into()];
    app.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
        selected: 0,
        active: None,
    }));
    app.activate_config_picker_item(0);
    let request = app.pending_switch.as_ref().expect("probe pending").request;
    app.handle_event(AppEvent::SwitchProbeFinished {
        request,
        result: Ok(FeedPage {
            posts: vec![post("probe")],
            after: None,
            has_more: false,
        }),
    });
    assert!(app.modal.is_none());
    let (loaded, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.provider.kind, crate::config::ProviderKind::Demo);
    assert_eq!(loaded.favorites, vec!["rust"]);

    // Exiting must not restore the old provider over the newly installed file.
    app.persist();
    let (persisted, warnings) = Settings::load(&path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(persisted.provider.kind, crate::config::ProviderKind::Demo);
    assert_eq!(persisted.favorites, vec!["rust"]);
    let backups = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".bckp_"))
        .count();
    assert_eq!(backups, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn theme_picker_previews_and_esc_restores() {
    let mut app = App::test_with_posts(vec![post("p")]);
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Dark);
    app.open_settings();
    app.activate_settings_item(1);
    assert!(matches!(app.modal, Some(Modal::ThemePicker(_))));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Light);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Dark);
    assert!(matches!(app.modal, Some(Modal::Settings(_))));
}

#[test]
fn theme_picker_enter_commits_and_persists() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-theme-picker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.config_path = dir.join("config.toml");
    app.open_settings();
    app.activate_settings_item(1);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Light);
    assert_eq!(app.persist_theme, crate::config::ThemeMode::Light);
    let (loaded, _) = Settings::load(&app.config_path);
    assert_eq!(loaded.general.theme, crate::config::ThemeMode::Light);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_change_switches_provider_without_restart() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-provider-switch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.config_path = dir.join("config.toml");
    app.library_path = dir.join("library.toml");
    app.settings.favorites = vec!["rust".into()];
    app.settings.general.theme = crate::config::ThemeMode::Mocha;
    app.persist_theme = crate::config::ThemeMode::Mocha;
    let old_view = app.current_id().unwrap();

    app.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
        selected: 1,
        active: None,
    }));
    app.activate_config_picker_item(1);
    let request = app.pending_switch.as_ref().expect("probe pending").request;
    app.handle_event(AppEvent::SwitchProbeFinished {
        request,
        result: Ok(FeedPage {
            posts: vec![post("probe")],
            after: None,
            has_more: false,
        }),
    });

    assert!(app.modal.is_none());
    assert_eq!(app.provider.short_name(), "REDLIB");
    assert!(
        app.provider_label.starts_with("REDLIB"),
        "{}",
        app.provider_label
    );
    assert!(!app.demo);
    assert_eq!(app.nav.len(), 1, "navigation stack must be reset safely");
    assert!(matches!(app.current(), Some(View::Feed(_))));
    assert_eq!(app.settings.favorites, vec!["rust"]);
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Mocha);

    let (loaded, warnings) = Settings::load(&app.config_path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.provider.kind, crate::config::ProviderKind::Redlib);
    assert_eq!(loaded.favorites, vec!["rust"]);
    assert_eq!(loaded.general.theme, crate::config::ThemeMode::Mocha);

    // A response from the old provider/view must not leak into the new feed.
    app.handle_event(AppEvent::FeedLoaded {
        view: old_view,
        request: 1,
        append: false,
        result: Ok(FeedPage {
            posts: vec![post("stale")],
            after: None,
            has_more: false,
        }),
    });
    assert!(app.current_feed().unwrap().posts.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_provider_probe_keeps_the_previous_source_and_config() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-provider-fail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.config_path = dir.join("config.toml");
    app.settings.general.theme = crate::config::ThemeMode::Mocha;
    app.persist_theme = crate::config::ThemeMode::Mocha;
    let before_kind = app.settings.provider.kind;
    let before_label = app.provider_label.clone();
    app.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
        selected: 1,
        active: None,
    }));
    app.activate_config_picker_item(1);
    let request = app.pending_switch.as_ref().expect("probe pending").request;
    app.handle_event(AppEvent::SwitchProbeFinished {
        request,
        result: Err(ProviderError::Server(503, "unavailable".into())),
    });
    assert_eq!(app.settings.provider.kind, before_kind);
    assert_eq!(app.provider_label, before_label);
    assert!(matches!(app.modal, Some(Modal::ConfigPicker(_))));
    assert!(
        !app.config_path.exists(),
        "nothing may be persisted on failure"
    );
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("current source was kept"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_requests_feed_the_source_health_screen() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let id = app.current_id().unwrap();
    app.current_feed_mut().unwrap().request = Some(41);
    app.request_starts.insert(41, Instant::now());
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 41,
        append: false,
        result: Err(ProviderError::RateLimited {
            retry_after: Some(5),
        }),
    });
    assert_eq!(
        app.health.last_status,
        Some(crate::providers::ProviderStatus::RateLimited)
    );
    assert_eq!(app.health.requests, 1);
    assert_eq!(app.health.failures, 1);
    assert!(app.health.last_latency.is_some());
    assert!(app.health.last_error.is_some());
    assert!(app.request_starts.is_empty());
}

#[test]
fn bookmarks_persist_snapshot_and_open_from_the_list() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-bookmarks-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.library_path = dir.join("library.toml");

    app.toggle_bookmark_current();
    assert!(app.library.is_bookmarked("p"));
    assert_eq!(app.library.bookmarks.len(), 1);
    let (loaded, warnings) = crate::library::Library::load(&app.library_path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.bookmarks[0].post.id, "p");
    assert_eq!(loaded.bookmarks[0].post.title, "Post p");

    app.open_bookmarks();
    assert!(matches!(app.modal, Some(Modal::Bookmarks(_))));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.current(), Some(View::Post(_))));
    assert!(app.library.is_post_read("p"));

    app.toggle_bookmark_current();
    assert!(!app.library.is_bookmarked("p"));
    let (loaded, _) = crate::library::Library::load(&app.library_path);
    assert!(loaded.bookmarks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_bookmark_save_reverts_and_does_not_claim_success() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-save-fail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let blocker = dir.join("not-a-directory");
    std::fs::write(&blocker, "block").unwrap();
    app.library_path = blocker.join("library.toml");
    app.toggle_bookmark_current();
    assert!(!app.library.is_bookmarked("p"));
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.kind == NoticeKind::Error)
    );
    assert!(
        !app.notifications
            .iter()
            .any(|notice| notice.text.contains("Saved"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_filter_save_reverts_the_filter() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-filter-fail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let blocker = dir.join("not-a-directory");
    std::fs::write(&blocker, "block").unwrap();
    app.library_path = blocker.join("library.toml");
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::NONE));
    assert!(app.library.filters.subreddits.is_empty());
    assert!(app.last_filter_undo.is_none());
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.kind == NoticeKind::Error)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_settings_save_reverts_favorites() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-settings-fail-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let blocker = dir.join("not-a-directory");
    std::fs::write(&blocker, "block").unwrap();
    app.config_path = blocker.join("config.toml");
    app.persist_error_shown = false;
    app.settings.favorites.clear();
    app.toggle_favorite();
    assert!(app.settings.favorites.is_empty());
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.kind == NoticeKind::Error)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_state_toggles_and_persists() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.library_path = dir.join("library.toml");

    app.toggle_read_current();
    assert!(app.library.is_post_read("p"));
    let (loaded, _) = crate::library::Library::load(&app.library_path);
    assert!(loaded.is_post_read("p"));

    app.toggle_read_current();
    assert!(!app.library.is_post_read("p"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comment_navigation_keys_move_between_roots_and_parents() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut root = Comment::new("a", "alice", "Root");
    root.replies.push(Comment::new("b", "bob", "Child"));
    root.replies[0]
        .replies
        .push(Comment::new("c", "carol", "Grandchild"));
    let second = Comment::new("d", "dave", "Second root");
    let comments = vec![root, second];
    push_post_view(&mut app, comments);
    app.prepare();

    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    assert_eq!(
        app.current_post().unwrap().rows[app.current_post().unwrap().selected].id,
        "d"
    );
    app.handle_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE));
    assert_eq!(
        app.current_post().unwrap().rows[app.current_post().unwrap().selected].id,
        "a"
    );
    app.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    app.current_post_mut().unwrap().selected = 2;
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert_eq!(
        app.current_post().unwrap().rows[app.current_post().unwrap().selected].id,
        "b"
    );
}

#[test]
fn comment_collapse_all_and_op_only_change_rows() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut root = Comment::new("a", "alice", "Root");
    root.replies.push(Comment::new("b", "bob", "Child"));
    let second = Comment::new("d", "dave", "Second root");
    push_post_view(&mut app, vec![root, second]);
    app.prepare();

    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert_eq!(app.current_post().unwrap().rows.len(), 2);
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
    assert_eq!(app.current_post().unwrap().rows.len(), 3);

    app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE));
    assert!(app.current_post().unwrap().op_only);
    assert_eq!(app.current_post().unwrap().rows.len(), 1);
    assert_eq!(app.current_post().unwrap().rows[0].id, "a");
    app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE));
    assert!(!app.current_post().unwrap().op_only);
}

#[test]
fn next_unread_comment_marks_and_skips_read_rows() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut root = Comment::new("a", "alice", "Root");
    root.replies.push(Comment::new("b", "bob", "Read"));
    root.replies.push(Comment::new("c", "carol", "Unread"));
    push_post_view(&mut app, vec![root]);
    app.prepare();
    app.library.mark_comment_read("b");

    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let view = app.current_post().unwrap();
    assert_eq!(view.rows[view.selected].id, "c");
    assert!(app.library.is_comment_read("c"));
    assert!(
        app.library.is_comment_read("a"),
        "current row is marked read"
    );
}

#[test]
fn permalink_targets_follow_comment_focus() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut root = Comment::new("a", "alice", "Root");
    root.replies.push(Comment::new("b", "bob", "Child"));
    push_post_view(&mut app, vec![root]);
    app.prepare();
    app.focus = Focus::Comments;
    app.current_post_mut().unwrap().selected = 1;
    let (comment_url, label) = app.current_permalink().unwrap();
    assert_eq!(label, "Comment permalink");
    assert!(
        comment_url.ends_with("/r/rust/comments/x/b/"),
        "{comment_url}"
    );

    app.focus = Focus::PostBody;
    let (post_url, label) = app.current_permalink().unwrap();
    assert_eq!(label, "Post permalink");
    assert_eq!(post_url, "https://www.reddit.com/r/rust/comments/x");
}

fn push_post_view(app: &mut App, comments: Vec<Comment>) {
    let post = app.current_feed().unwrap().posts[0].clone();
    let id = app.alloc_view();
    app.nav.push(View::Post(PostView::with_comments(
        id,
        post,
        comments,
        CommentSort::Best,
    )));
    app.focus = Focus::Comments;
}

#[test]
fn settings_can_change_image_protocol_and_appearance() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_settings();
    app.activate_settings_item(2);
    assert!(matches!(app.modal, Some(Modal::ImagePicker(_))));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.settings.images.protocol,
        crate::config::ImageProtocolSetting::Kitty
    );
    assert_eq!(
        app.persist_images_protocol,
        crate::config::ImageProtocolSetting::Kitty
    );
    assert!(app.settings.images.enabled);

    app.activate_settings_item(3);
    assert!(matches!(app.modal, Some(Modal::Appearance(_))));
    let before_time = app.settings.ui.relative_time;
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_ne!(app.settings.ui.relative_time, before_time);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    let before_motion = app.settings.ui.reduce_motion;
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_ne!(app.settings.ui.reduce_motion, before_motion);
}

#[test]
fn source_health_screen_opens_from_settings_and_offers_retry() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_settings();
    app.activate_settings_item(4);
    assert!(matches!(app.modal, Some(Modal::Source(_))));
    app.execute_source_action(0);
    assert!(matches!(app.modal, Some(Modal::Source(_))));
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.text.contains("Testing"))
    );
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::ConfigPicker(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Settings(_))));
}

#[test]
fn local_filters_hide_loaded_posts_and_paginate_when_empty() {
    let mut app = App::test_with_posts((0..4).map(|i| post(&format!("p{i}"))).collect());
    app.library
        .filters
        .add_expression("keyword:Post p1")
        .unwrap();
    let id = app.current_id().unwrap();
    app.current_feed_mut().unwrap().request = Some(77);
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 77,
        append: false,
        result: Ok(FeedPage {
            posts: (0..4).map(|i| post(&format!("p{i}"))).collect(),
            after: Some("next".into()),
            has_more: true,
        }),
    });
    let feed = app.current_feed().unwrap();
    assert!(feed.posts.iter().all(|post| post.id != "p1"));
    assert_eq!(feed.posts.len(), 3);
    assert!(feed.has_more);
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.text.contains("1 post(s) hidden"))
    );
}

#[test]
fn hide_read_filter_drops_read_posts() {
    let mut app = App::test_with_posts((0..3).map(|i| post(&format!("p{i}"))).collect());
    app.library.filters.hide_read = true;
    app.library.mark_post_read("p0");
    let id = app.current_id().unwrap();
    app.current_feed_mut().unwrap().request = Some(78);
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 78,
        append: false,
        result: Ok(FeedPage {
            posts: (0..3).map(|i| post(&format!("p{i}"))).collect(),
            after: None,
            has_more: false,
        }),
    });
    assert_eq!(app.current_feed().unwrap().posts.len(), 2);
    assert!(app.library.is_post_read("p0"));
}

#[test]
fn filters_modal_adds_removes_and_clears_rules() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.library_path = std::env::temp_dir().join(format!(
        "reddtty-filters-{}-{}.toml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.open_settings();
    app.activate_settings_item(5);
    assert!(matches!(app.modal, Some(Modal::Filters(_))));
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Text(_))));
    for c in "keyword:Post".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.library.filters.keywords, vec!["Post"]);
    assert!(matches!(app.modal, Some(Modal::Filters(_))));

    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(app.library.filters.keywords.is_empty());

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.library.filters.hide_read);
    app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.library.filters.is_active());
    let (loaded, _) = crate::library::Library::load(&app.library_path);
    assert!(!loaded.filters.is_active());
    let _ = std::fs::remove_file(&app.library_path);
}

#[test]
fn command_palette_runs_selected_actions() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::CommandPalette(_))));
    for c in "mocha".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Mocha);
    assert!(app.modal.is_none());

    app.open_command_palette();
    for c in "filters".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Filters(_))));

    // Typing q inside the palette must filter, never quit the app.
    app.open_command_palette();
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(!app.should_quit);
    assert!(matches!(app.modal, Some(Modal::CommandPalette(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.modal.is_none());
}

#[test]
fn command_palette_runs_text_aliases() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_command_palette();
    for c in "theme mocha".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Mocha);
    app.open_command_palette();
    for c in "theme nope".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let Some(Modal::CommandPalette(palette)) = &app.modal else {
        panic!("palette should stay open on an invalid alias");
    };
    assert!(
        palette
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Usage"))
    );

    app.open_command_palette();
    for c in "monochrome".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Mono);

    app.open_command_palette();
    for c in "high contrast".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.settings.general.theme,
        crate::config::ThemeMode::HighContrast
    );

    // Incomplete alias text falls back to the highlighted action.
    app.open_command_palette();
    for c in "theme".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    for _ in 0..6 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.settings.general.theme, crate::config::ThemeMode::Mocha);

    app.open_command_palette();
    for c in "theme Ma".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.settings.general.theme,
        crate::config::ThemeMode::Macchiato
    );
}

#[test]
fn quick_mute_and_undo_update_local_filters() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-mute-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.library_path = dir.join("library.toml");
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::NONE));
    assert_eq!(app.library.filters.subreddits, vec!["rust"]);
    assert!(app.last_filter_undo.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    assert!(app.library.filters.subreddits.is_empty());
    assert!(app.last_filter_undo.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ctrl_p_toggles_the_command_palette() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(matches!(app.modal, Some(Modal::CommandPalette(_))));
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.modal.is_none());
}

#[test]
fn link_actions_target_post_subreddit_and_image() {
    let mut image = post("p");
    image.permalink = "/r/rust/comments/abc/a_title/".into();
    image.content = PostContent::Image {
        url: "https://images.example/a.png".into(),
    };
    let mut app = App::test_with_posts(vec![image.clone()]);
    assert_eq!(
        app.comments_url().unwrap(),
        "https://www.reddit.com/r/rust/comments/abc/a_title/"
    );
    assert_eq!(
        app.subreddit_url().unwrap(),
        "https://www.reddit.com/r/rust"
    );
    assert_eq!(
        app.current_image_url().unwrap(),
        "https://images.example/a.png"
    );

    let id = app.alloc_view();
    let gallery = crate::app::gallery::GalleryView::new(id, image).unwrap();
    app.nav.push(View::Gallery(gallery));
    assert_eq!(
        app.current_image_url().unwrap(),
        "https://images.example/a.png"
    );
    assert_eq!(
        app.comments_url().unwrap(),
        "https://www.reddit.com/r/rust/comments/abc/a_title/"
    );

    app.nav.pop();
    app.focus = Focus::Sidebar;
    let index = app
        .sidebar_entries()
        .iter()
        .position(|entry| entry.label == "r/linux")
        .expect("r/linux is suggested");
    app.sidebar.selected = index;
    assert_eq!(
        app.subreddit_url().unwrap(),
        "https://www.reddit.com/r/linux"
    );
}

#[test]
fn demo_image_urls_are_not_offered_as_external_links() {
    let mut image = post("p");
    image.content = PostContent::Image {
        url: "demo://img/a.png".into(),
    };
    let mut app = App::test_with_posts(vec![image]);
    assert_eq!(app.current_image_url().unwrap(), "demo://img/a.png");
    app.handle_key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE));
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.text.contains("local fixtures")),
        "{:?}",
        app.notifications
    );
}

#[test]
fn invalid_subreddits_do_not_produce_browser_urls() {
    let mut bad = post("p");
    bad.subreddit = "bad name!".into();
    let app = App::test_with_posts(vec![bad]);
    assert!(app.subreddit_url().is_none());
    assert!(
        app.comments_url().is_some(),
        "the post itself is still valid"
    );
}

#[test]
fn info_drawer_snapshots_community_and_post_details() {
    let mut app = App::test_with_posts(vec![post("p")]);
    if let Some(feed) = app.current_feed_mut() {
        feed.subreddit = Some(Subreddit {
            name: "rust".into(),
            title: "The Rust Programming Language".into(),
            description: "Systems programming".into(),
            subscribers: 100,
            active_users: 5,
            over18: false,
            created_utc: 0,
        });
    }
    app.open_info();
    let Some(Modal::Info(modal)) = &app.modal else {
        panic!("expected info drawer");
    };
    assert_eq!(modal.info.title, "Home");
    assert_eq!(modal.info.members, Some((100, 5)));
    assert!(modal.info.actions.iter().any(|(key, _)| key == "g"));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.modal.is_none());

    app.open_post();
    if let Some(post) = app.current_post_mut() {
        post.subreddit = Some(Subreddit::new("rust"));
    }
    app.open_info();
    let Some(Modal::Info(modal)) = &app.modal else {
        panic!("expected info drawer");
    };
    assert_eq!(modal.info.title, "r/rust");
    assert!(
        modal
            .info
            .post_lines
            .iter()
            .any(|line| line.contains("points"))
    );
}

#[test]
fn info_drawer_actions_execute_immediately() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_info();
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert!(app.modal.is_none());
    assert!(app.library.is_bookmarked("p"));

    app.open_info();
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert!(
        app.settings
            .is_favorite(&app.current_subreddit_name().unwrap_or_default())
            || app.modal.is_none()
    );

    app.open_info();
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Text(_))));

    app.modal = None;
    app.open_info();
    app.handle_key(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT));
    assert!(app.modal.is_none());
    assert!(
        app.notifications
            .iter()
            .any(|notice| { notice.text.contains("comments") || notice.text.contains("browser") })
    );
}

#[test]
fn gallery_shift_shortcuts_are_not_swallowed_by_panning() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut image = post("g");
    image.content = PostContent::Image {
        url: "demo://img/a.png".into(),
    };
    let id = app.alloc_view();
    app.nav.push(View::Gallery(
        crate::app::gallery::GalleryView::new(id, image).unwrap(),
    ));
    app.set_size(100, 30);
    app.prepare();

    let before = app.settings.general.theme;
    app.handle_key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT));
    assert_ne!(app.settings.general.theme, before);

    app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Info(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT));
    assert!(
        app.modal.is_none(),
        "uppercase I copies instead of opening info"
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT));
    assert!(matches!(app.modal, Some(Modal::Bookmarks(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT));
    assert_eq!(app.current_gallery().unwrap().view.zoom, 150);

    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
    assert!(matches!(app.modal, Some(Modal::Help(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::SHIFT));
    assert!(matches!(app.modal, Some(Modal::CommandPalette(_))));
}

#[test]
fn arrows_move_between_sidebar_and_feed() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.focus = Focus::Feed;
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Sidebar);
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Feed);
}

#[test]
fn left_arrow_shows_a_hidden_sidebar_on_wide_terminals() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.settings.general.show_sidebar = false;
    app.focus = Focus::Feed;
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert!(app.settings.general.show_sidebar);
    assert_eq!(app.focus, Focus::Sidebar);
}

#[test]
fn left_arrow_explains_narrow_terminals_without_a_sidebar() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.set_size(60, 24);
    app.focus = Focus::Feed;
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Feed);
    assert!(
        app.notifications
            .iter()
            .any(|notice| notice.text.contains("76 columns")),
        "{:?}",
        app.notifications
    );
}

#[test]
fn arrows_move_between_post_body_and_comments() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let mut root = Comment::new("a", "alice", "Root");
    root.replies.push(Comment::new("b", "bob", "Child"));
    push_post_view(&mut app, vec![root]);
    app.prepare();
    assert_eq!(app.focus, Focus::Comments);
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::PostBody);
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.focus, Focus::Comments);
}

#[test]
fn post_detail_receives_its_community_context() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let subreddit = Subreddit {
        name: "rust".into(),
        title: "The Rust Programming Language".into(),
        description: "Systems programming".into(),
        subscribers: 10,
        active_users: 2,
        over18: false,
        created_utc: 0,
    };
    if let Some(feed) = app.current_feed_mut() {
        feed.subreddit = Some(subreddit.clone());
    }
    app.open_post();
    assert_eq!(
        app.current_post().unwrap().subreddit.as_ref().unwrap().name,
        "rust"
    );

    let id = app.current_id().unwrap();
    if let Some(post) = app.current_post_mut() {
        post.subreddit_request = Some(55);
    }
    app.request_starts.insert(55, Instant::now());
    app.handle_event(AppEvent::SubredditLoaded {
        view: id,
        request: 55,
        name: "rust".into(),
        result: Ok(Subreddit {
            description: "Fresh description".into(),
            ..subreddit
        }),
    });
    let post = app.current_post().unwrap();
    assert_eq!(
        post.subreddit.as_ref().unwrap().description,
        "Fresh description"
    );
    assert!(post.subreddit_request.is_none());
    assert_eq!(app.health.requests, 1);
}

#[test]
fn breadcrumb_tracks_the_navigation_stack() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_feed(FeedSpec::Subreddit("rust".into()), None, true);
    if let Some(feed) = app.current_feed_mut() {
        feed.posts = vec![post("p")];
    }
    assert_eq!(app.breadcrumb(), "Home › r/rust");
    app.open_post();
    assert!(
        app.breadcrumb().ends_with("r/rust › Post"),
        "{}",
        app.breadcrumb()
    );
}

#[test]
fn new_restores_the_last_feed_and_sort() {
    let dir = std::env::temp_dir().join(format!(
        "reddtty-last-view-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.toml");
    let library = crate::library::Library {
        last_feed: Some(FeedSpec::Subreddit("rust".into())),
        last_sort: Some(FeedSort::New),
        ..crate::library::Library::default()
    };
    library
        .save(&crate::library::Library::path_for(&config_path))
        .unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let app = App::new(
        Arc::new(TestProvider),
        Settings::default(),
        config_path,
        ImageManager::disabled(tx.clone()),
        tx,
        None,
        None,
        Vec::new(),
    );
    let feed = app.current_feed().unwrap();
    assert_eq!(feed.spec, FeedSpec::Subreddit("rust".into()));
    assert_eq!(feed.sort, FeedSort::New);
    let _ = std::fs::remove_dir_all(&dir);
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
    // A stale result from an old view cannot relabel the UI or inject posts.
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
    assert!(
        feed.has_more,
        "the cursor must survive so retry is possible"
    );
    assert!(feed.more_request.is_none());
    assert!(feed.more_error.is_some());
    assert!(
        !feed.wants_more(24),
        "automatic pagination must stay paused"
    );
}

#[test]
fn failed_append_can_be_retried_without_refreshing_the_feed() {
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
    let before: Vec<String> = app
        .current_feed()
        .unwrap()
        .posts
        .iter()
        .map(|p| p.id.clone())
        .collect();
    app.retry_more();
    let feed = app.current_feed().unwrap();
    assert_eq!(
        feed.posts.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        before
    );
    assert!(feed.more_error.is_none());
    assert!(
        feed.more_request.is_some(),
        "retry issues exactly one request"
    );
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

#[test]
fn bookmark_search_navigation_and_delete_confirmation_preserve_state() {
    let mut app = App::test_with_posts(Vec::new());
    for (id, title, author, subreddit) in [
        ("b1", "Rust news", "alice", "rust"),
        ("b2", "Cooking tips", "bob", "cooking"),
        ("b3", "Rust macros", "carol", "rust"),
    ] {
        let mut item = post(id);
        item.title = title.into();
        item.author = author.into();
        item.subreddit = subreddit.into();
        app.library.toggle_bookmark(&item);
    }
    app.open_bookmarks();
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
    let Some(Modal::Bookmarks(modal)) = &app.modal else {
        panic!("expected bookmarks");
    };
    assert_eq!(modal.query, "ru");
    assert_eq!(modal.filtered(&app.library).len(), 2);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.nav.last(), Some(View::Post(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let Some(Modal::Bookmarks(modal)) = &app.modal else {
        panic!("bookmark list should return with its filter");
    };
    assert_eq!(modal.query, "ru");
    assert_eq!(modal.selected, 1);

    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Bookmarks(_))));
    assert_eq!(app.library.bookmarks.len(), 3);

    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.library.bookmarks.len(), 2);
    assert!(matches!(app.modal, Some(Modal::Bookmarks(_))));
}

#[test]
fn config_picker_starts_on_active_source_and_offers_custom_instance() {
    let mut app = App::test_with_posts(Vec::new());
    let picker = ConfigPickerModal::for_settings(&app.settings);
    assert_eq!(picker.active, None, "auto is not a bundled preset");
    app.settings.provider.kind = crate::config::ProviderKind::Demo;
    let picker = ConfigPickerModal::for_settings(&app.settings);
    assert_eq!(picker.active, Some(0));
    assert_eq!(picker.selected, 0);
}

#[test]
fn custom_redlib_requires_confirmation_and_persists_explicit_consent() {
    let mut app = App::test_with_posts(Vec::new());
    app.open_custom_redlib();
    for ch in "redlib.example".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    // Cancelling returns to the form without changing settings.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Text(_))));
    assert_eq!(
        app.settings.provider.kind,
        crate::config::ProviderKind::Auto
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    // The candidate is probed before anything is persisted or activated.
    let request = app.pending_switch.as_ref().expect("probe pending").request;
    assert_eq!(
        app.settings.provider.kind,
        crate::config::ProviderKind::Auto
    );
    app.handle_event(AppEvent::SwitchProbeFinished {
        request,
        result: Ok(FeedPage {
            posts: vec![post("probe")],
            after: None,
            has_more: false,
        }),
    });
    assert_eq!(
        app.settings.provider.kind,
        crate::config::ProviderKind::Redlib
    );
    assert_eq!(
        app.settings.provider.html.redlib_instance,
        "https://redlib.example"
    );
    assert!(app.settings.provider.html.allow_network);
    assert!(app.modal.is_none());
}

#[test]
fn source_probe_never_replaces_the_visible_feed() {
    let mut app = App::test_with_posts(vec![post("p1"), post("p2")]);
    let before: Vec<String> = app
        .current_feed()
        .unwrap()
        .posts
        .iter()
        .map(|p| p.id.clone())
        .collect();
    app.probe_request = Some(77);
    app.handle_probe_finished(
        77,
        Ok(FeedPage {
            posts: vec![post("other")],
            after: None,
            has_more: false,
        }),
    );
    assert_eq!(
        app.current_feed()
            .unwrap()
            .posts
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(app.health.requests, 1);
    assert!(app.probe_request.is_none());
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("probe OK"))
    );
}

#[test]
fn failed_source_probe_is_reported_and_recorded() {
    let mut app = App::test_with_posts(vec![post("p1")]);
    app.probe_request = Some(78);
    app.handle_probe_finished(78, Err(ProviderError::Server(500, "down".into())));
    assert_eq!(app.health.failures, 1);
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("probe failed"))
    );
}

#[test]
fn clearing_filters_requires_confirmation_and_cancel_keeps_them() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.library.filters.subreddits.push("politics".into());
    app.open_filters();
    app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.library.filters.subreddits, vec!["politics"]);
    assert!(matches!(app.modal, Some(Modal::Filters(_))));
    app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.library.filters.subreddits.is_empty());
}

#[test]
fn clearing_read_history_is_confirmed_and_persisted() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.library.mark_post_read("p");
    app.open_settings();
    app.activate_settings_item(6);
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.library.is_post_read("p"));
    let (loaded, _) = crate::library::Library::load(&app.library_path);
    assert!(!loaded.is_post_read("p"));
}

#[test]
fn domain_mute_rejects_self_posts_instead_of_creating_a_subreddit_filter() {
    let mut app = App::test_with_posts(vec![post("self")]);
    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT));
    assert!(app.library.filters.domains.is_empty());
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("no external domain"))
    );
}

#[test]
fn cli_network_permission_is_not_persisted_by_an_unrelated_setting_change() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-session-provider-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.config_path = dir.join("config.toml");
    app.persist_provider.html.allow_network = false;
    app.settings.provider.html.allow_network = true;
    app.toggle_theme();
    let (loaded, warnings) = Settings::load(&app.config_path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(
        !loaded.provider.html.allow_network,
        "a CLI-only permission must not be written to disk"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_config_blocks_unrelated_saves_until_explicit_recovery() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.config_path = dir.join("config.toml");
    std::fs::write(&app.config_path, "not [ valid").unwrap();
    app.config_corrupt = true;
    app.config_recovery = Some(dir.join("config.toml.corrupt-test"));
    std::fs::write(dir.join("config.toml.corrupt-test"), "not [ valid").unwrap();
    app.persist_error_shown = false;
    app.toggle_theme();
    assert_eq!(std::fs::read(&app.config_path).unwrap(), b"not [ valid");
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("Configuration is corrupt"))
    );

    app.open_settings();
    app.activate_settings_item(7);
    assert!(matches!(app.modal, Some(Modal::Recovery(_))));
    app.execute_recovery_action(2);
    assert!(!app.config_corrupt);
    assert!(app.should_quit);
    let (loaded, warnings) = Settings::load(&app.config_path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.general.theme, app.settings.general.theme);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn consecutive_settings_saves_do_not_self_conflict() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let dir = std::env::temp_dir().join(format!(
        "reddtty-revision-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.config_path = dir.join("config.toml");
    app.persist_error_shown = false;
    app.toggle_theme();
    assert!(app.persist());
    app.toggle_relative_time();
    assert!(app.persist(), "{:?}", app.notifications);
    let (loaded, warnings) = Settings::load(&app.config_path);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.revision, app.settings.revision);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn probe_request_clamps_best_to_a_sort_the_html_provider_supports() {
    let mut settings = Settings::default();
    settings.general.default_sort = "best".into();
    let provider = crate::providers::html::old_reddit::OldRedditProvider::new(
        crate::providers::html::client::HtmlClientConfig::new("http://127.0.0.1:9", "reddtty-test"),
    )
    .unwrap();
    let request = probe_feed_request(&settings, &provider);
    assert_ne!(request.sort, FeedSort::Best);
    assert!(
        provider
            .supported_sorts(&request.spec)
            .contains(&request.sort),
        "probe sort {:?} is not supported",
        request.sort
    );
}

#[test]
fn late_probe_from_the_old_provider_is_ignored_after_reload() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.probe_request = Some(501);
    app.reload_provider();
    assert!(app.probe_request.is_none());
    app.handle_probe_finished(
        501,
        Ok(FeedPage {
            posts: vec![post("stale")],
            after: None,
            has_more: false,
        }),
    );
    assert_eq!(app.health.requests, 0);
    assert!(
        app.notifications
            .iter()
            .all(|n| !n.text.contains("probe OK"))
    );
}

#[test]
fn cancelling_a_source_test_ignores_its_late_success() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let before = app.provider_label.clone();
    app.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
        selected: 1,
        active: None,
    }));
    app.activate_config_picker_item(1);
    assert!(app.pending_switch.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.pending_switch.is_none());
    app.handle_event(AppEvent::SwitchProbeFinished {
        request: 1,
        result: Ok(FeedPage {
            posts: vec![post("late")],
            after: None,
            has_more: false,
        }),
    });
    assert_eq!(app.provider_label, before);
    assert_eq!(
        app.settings.provider.kind,
        crate::config::ProviderKind::Auto
    );
}

#[test]
fn two_app_sessions_merge_bookmarks_through_the_real_save_path() {
    let dir = std::env::temp_dir().join(format!(
        "reddtty-app-merge-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let library_path = dir.join("library.toml");
    crate::library::Library::default()
        .save(&library_path)
        .unwrap();
    let mut first = App::test_with_posts(vec![post("p")]);
    let mut second = App::test_with_posts(vec![post("p")]);
    first.library_path = library_path.clone();
    second.library_path = library_path.clone();
    first.library = crate::library::Library::load(&library_path).0;
    second.library = crate::library::Library::load(&library_path).0;
    first.library.toggle_bookmark(&post("from-first"));
    assert!(first.save_library());
    second.library.toggle_bookmark(&post("from-second"));
    assert!(second.save_library());
    let (disk, _) = crate::library::Library::load(&library_path);
    assert!(disk.is_bookmarked("from-first"));
    assert!(disk.is_bookmarked("from-second"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restored_backup_survives_exit_persistence() {
    let dir = std::env::temp_dir().join(format!(
        "reddtty-restore-exit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let backup = dir.join("config.toml.bckp_1");
    std::fs::write(&path, "not [ valid").unwrap();
    let mut restored = Settings::default();
    restored.general.theme = crate::config::ThemeMode::Mocha;
    let backup_bytes = toml::to_string_pretty(&restored).unwrap().into_bytes();
    std::fs::write(&backup, &backup_bytes).unwrap();
    let mut app = App::test_with_posts(vec![post("p")]);
    app.config_path = path.clone();
    app.config_corrupt = true;
    app.config_recovery = Some(dir.join("config.toml.corrupt-1"));
    app.execute_recovery_action(1);
    assert!(app.should_quit);
    app.finalize_on_exit();
    assert_eq!(std::fs::read(&path).unwrap(), backup_bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_restores_its_cursor_after_a_submenu() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.open_settings();
    for _ in 0..5 {
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.modal, Some(Modal::Filters(_))));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let Some(Modal::Settings(modal)) = &app.modal else {
        panic!("expected settings");
    };
    assert_eq!(modal.selected, 5);
}

#[test]
fn no_color_blocks_theme_and_image_protocol_changes() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.no_color = true;
    let before = app.settings.general.theme;
    app.toggle_theme();
    assert_eq!(app.settings.general.theme, before);
    app.commit_image_protocol(crate::config::ImageProtocolSetting::Halfblocks);
    assert!(!app.images.enabled());
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("NO_COLOR"))
    );
}

#[test]
fn custom_redlib_has_no_false_active_preset() {
    let mut app = App::test_with_posts(vec![post("p")]);
    app.settings.provider.kind = crate::config::ProviderKind::Redlib;
    app.settings.provider.html.redlib_instance = "https://redlib.example".into();
    let picker = ConfigPickerModal::for_settings(&app.settings);
    assert_eq!(picker.active, None);
}

#[test]
fn retry_after_blocks_manual_retry_until_the_cooldown_expires() {
    let mut app = App::test_with_posts((0..3).map(|i| post(&format!("p{i}"))).collect());
    let id = app.current_id().unwrap();
    {
        let feed = app.current_feed_mut().unwrap();
        feed.has_more = true;
        feed.after = Some("d:3".into());
        feed.more_request = Some(31);
    }
    app.handle_event(AppEvent::FeedLoaded {
        view: id,
        request: 31,
        append: true,
        result: Err(ProviderError::RateLimited {
            retry_after: Some(30),
        }),
    });
    let wait = app.current_feed().unwrap().retry_wait_secs().unwrap();
    assert!((29..=30).contains(&wait), "{wait}");
    app.retry_more();
    assert!(app.current_feed().unwrap().more_request.is_none());
    assert!(
        app.notifications
            .iter()
            .any(|n| n.text.contains("retry available in"))
    );
}

#[test]
fn clicking_outside_the_source_picker_cancels_the_candidate() {
    let mut app = App::test_with_posts(vec![post("p")]);
    let before = app.provider_label.clone();
    app.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
        selected: 1,
        active: None,
    }));
    app.activate_config_picker_item(1);
    let request = app.pending_switch.as_ref().unwrap().request;
    app.handle_modal_click(None);
    assert!(app.pending_switch.is_none());
    app.handle_event(AppEvent::SwitchProbeFinished {
        request,
        result: Ok(FeedPage {
            posts: vec![post("late")],
            after: None,
            has_more: false,
        }),
    });
    assert_eq!(app.provider_label, before);
}
