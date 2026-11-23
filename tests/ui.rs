use std::path::PathBuf;
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use reddtty::app::gallery::GalleryView;
use reddtty::app::post::PostView;
use reddtty::app::{App, Focus, View};
use reddtty::config::Settings;
use reddtty::event::ViewId;
use reddtty::media::ImageManager;
use reddtty::models::{Comment, CommentSort, FeedRequest, FeedSort, FeedSpec, Post, TopRange};
use reddtty::providers::ContentProvider;
use reddtty::providers::demo::DemoProvider;
use reddtty::ui;
use tokio::sync::mpsc::unbounded_channel;

fn test_config_path() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "reddtty-test-{}-{}.toml",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn settings() -> Settings {
    let mut settings = Settings::default();
    settings.images.enabled = false;
    settings.general.show_sidebar = true;
    settings.ui.show_right_panel = true;
    settings
}

async fn demo() -> (Arc<DemoProvider>, Vec<Post>) {
    let provider = Arc::new(DemoProvider::new(&Settings::cache_dir()));
    let page = provider
        .get_feed(FeedRequest::first(
            FeedSpec::Home,
            FeedSort::Hot,
            TopRange::Week,
            60,
        ))
        .await
        .unwrap();
    (provider, page.posts)
}

async fn app_with_posts(width: u16, height: u16) -> (App, Arc<DemoProvider>, Vec<Post>) {
    let (provider, posts) = demo().await;
    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let config = test_config_path();
    let mut app = App::new(
        provider.clone(),
        settings,
        config,
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    if let Some(feed) = app.current_feed_mut() {
        feed.posts = posts.clone();
        feed.loading_initial = false;
        feed.heights_dirty = true;
        feed.error = None;
        feed.has_more = false;
    }
    app.set_size(width, height);
    app.prepare();
    (app, provider, posts)
}

fn draw(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app))
        .expect("render must not fail");
    terminal
}

fn screen(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[tokio::test]
async fn renders_across_terminal_sizes() {
    for (w, h) in [
        (20, 6),
        (30, 10),
        (60, 20),
        (80, 24),
        (100, 30),
        (130, 36),
        (140, 40),
        (180, 50),
    ] {
        let (mut app, _provider, _posts) = app_with_posts(w, h).await;
        app.prepare();
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        assert!(text.contains("REDDTTY"), "missing brand at {w}x{h}");
        if w >= 40 {
            assert!(
                text.contains("Home"),
                "missing feed title at {w}x{h}: {text:?}"
            );
        }
        assert!(text.lines().count() >= h as usize);
    }
}

#[tokio::test]
async fn responsive_sidebar_and_right_panel() {
    let (mut app, _provider, _posts) = app_with_posts(60, 20).await;
    let narrow = screen(&draw(&mut app, 60, 20));
    assert!(!narrow.contains("NAVIGATION"));

    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    let medium = screen(&draw(&mut app, 100, 30));
    assert!(medium.contains("NAVIGATION"));
    assert!(!medium.contains("SUGGESTED"));

    let (mut app, _provider, _posts) = app_with_posts(160, 40).await;
    let wide = screen(&draw(&mut app, 160, 40));
    assert!(wide.contains("ABOUT"));
    assert!(wide.contains("SUGGESTED"));
}

#[tokio::test]
async fn feed_renders_cards_and_footer() {
    let (mut app, _provider, posts) = app_with_posts(110, 32).await;
    let terminal = draw(&mut app, 110, 32);
    let text = screen(&terminal);
    assert!(text.contains(&posts[0].title[..posts[0].title.len().min(18)]));
    assert!(text.contains("▲"));
    assert!(text.contains("Hot"));
}

#[tokio::test]
async fn post_detail_renders_post_and_nested_comments() {
    let (provider, mut posts) = demo().await;
    let post = posts
        .iter()
        .find(|p| p.id == "demo001")
        .cloned()
        .unwrap_or_else(|| posts.remove(0));
    let comments = provider
        .get_comments(&post.id, CommentSort::Best)
        .await
        .unwrap();

    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        provider.clone(),
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    let id = ViewId(900);
    let mut view = PostView::new(id, post.clone(), CommentSort::Best);
    view.comments = comments;
    view.comments_loading = false;
    view.comments_dirty = true;
    app.nav.push(View::Post(view));
    app.focus = Focus::Comments;
    app.set_size(120, 40);
    app.prepare();
    let terminal = draw(&mut app, 120, 40);
    let text = screen(&terminal);
    assert!(text.contains("COMMENTS"));
    assert!(
        text.contains("├─"),
        "nested comments should show tree rails"
    );
    assert!(text.contains("Sort: Best"));
}

#[tokio::test]
async fn gallery_renders_position_and_controls() {
    let (_provider, posts) = demo().await;
    let post = posts
        .iter()
        .find(|p| p.has_gallery())
        .cloned()
        .expect("demo data should contain a gallery");
    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        Arc::new(DemoProvider::new(&Settings::cache_dir())),
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    let gallery = GalleryView::new(ViewId(800), post.clone()).unwrap();
    let count = gallery.images.len();
    app.nav.push(View::Gallery(gallery));
    app.set_size(100, 30);
    app.prepare();
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("Image Viewer"));
    assert!(text.contains(&format!("1 / {count}")));
    assert!(text.contains("Esc close"));
    assert!(
        text.lines().last().unwrap_or("").contains("Quit"),
        "gallery status bar lost the quit hint"
    );
    let _ = post;
}

#[tokio::test]
async fn help_and_search_modals_fit_small_screens() {
    for (w, h) in [(60, 20), (80, 24)] {
        let (mut app, _provider, _posts) = app_with_posts(w, h).await;
        app.open_help();
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        assert!(text.contains("Keyboard Shortcuts"));
        assert!(matches!(app.modal, Some(reddtty::app::Modal::Help)));

        app.modal = None;
        app.open_search_modal();
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        assert!(text.contains("Search"));
        assert!(text.contains("Scope:"));
    }
}

#[tokio::test]
async fn empty_feed_shows_error_state_with_retry() {
    let (mut app, _provider, _posts) = app_with_posts(90, 26).await;
    if let Some(feed) = app.current_feed_mut() {
        feed.posts.clear();
        feed.loading_initial = false;
        feed.error = Some("Network request failed.".into());
        feed.heights_dirty = true;
    }
    app.prepare();
    let terminal = draw(&mut app, 90, 26);
    let text = screen(&terminal);
    assert!(text.contains("Unable to load posts."));
    assert!(text.contains("Retry"));
}

#[tokio::test]
async fn hidden_post_renders_warning_not_content() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    if let Some(feed) = app.current_feed_mut() {
        if let Some(post) = feed.posts.first_mut() {
            post.nsfw = true;
            post.body = "secret body".into();
        }
        feed.selected = 0;
        feed.heights_dirty = true;
    }
    app.prepare();
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("content hidden"));
    assert!(!text.contains("secret body"));
}

#[tokio::test]
async fn sort_modal_opens_and_renders_supported_sorts() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    app.open_sort_modal();
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("Sort"));
    assert!(text.contains("Rising"));
    assert!(text.contains("Top time range"));
}

#[tokio::test]
async fn comments_survive_narrow_render() {
    let (provider, posts) = demo().await;
    let post = posts.iter().find(|p| p.id == "demo001").cloned().unwrap();
    let comments: Vec<Comment> = provider
        .get_comments(&post.id, CommentSort::Best)
        .await
        .unwrap();
    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        provider.clone(),
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    let mut view = PostView::new(ViewId(700), post, CommentSort::Best);
    view.comments = comments;
    view.comments_loading = false;
    view.comments_dirty = true;
    app.nav.push(View::Post(view));
    app.focus = Focus::Comments;
    for (w, h) in [(60, 20), (80, 24), (120, 40)] {
        app.set_size(w, h);
        app.prepare();
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        assert!(text.contains("COMMENTS"));
        let last = text.lines().last().unwrap_or("");
        assert!(
            last.contains("Quit"),
            "post status bar at {w}x{h} lost the quit hint: {last:?}"
        );
    }
}

#[tokio::test]
async fn initial_feed_request_actually_loads_posts() {
    let (provider, _posts) = demo().await;
    let mut settings = Settings::default();
    settings.images.enabled = false;
    let (tx, mut rx) = unbounded_channel();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        provider,
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    assert_eq!(
        app.current_feed().unwrap().sort,
        FeedSort::Best,
        "default Home sort should be Best"
    );
    let mut loaded = false;
    for _ in 0..5 {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("event must arrive")
            .expect("channel open");
        app.handle_event(event);
        if app.current_feed().is_some_and(|f| !f.posts.is_empty()) {
            loaded = true;
            break;
        }
    }
    assert!(loaded, "feed never loaded posts");
    assert!(!app.current_feed().unwrap().loading_initial);
}

#[tokio::test]
async fn mouse_click_selects_and_double_click_opens_post() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    let terminal = draw(&mut app, 120, 36);
    let _ = terminal;
    let rect = app
        .hit_map
        .iter()
        .find(|area| area.target == reddtty::app::hit::HitTarget::FeedPost(1))
        .map(|area| area.rect)
        .expect("second card should be visible");
    let (x, y) = (rect.x + 4, rect.y + 2);
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: x,
        row: y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.current_feed().unwrap().selected, 1);
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: x,
        row: y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.nav.len(), 2, "double click should open the post");
    assert!(matches!(app.current(), Some(View::Post(_))));
}

#[tokio::test]
async fn mouse_wheel_scrolls_feed() {
    let (mut app, _provider, _posts) = app_with_posts(120, 30).await;
    let _terminal = draw(&mut app, 120, 30);
    let pane = app
        .hit_map
        .iter()
        .find(|area| {
            area.target == reddtty::app::hit::HitTarget::Pane(reddtty::app::hit::Pane::FeedList)
        })
        .map(|area| area.rect)
        .expect("feed pane hitbox");
    let before = app.current_feed().unwrap().offset;
    for _ in 0..5 {
        app.handle_mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollDown,
            column: pane.x + 5,
            row: pane.y + 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
    }
    let feed = app.current_feed().unwrap();
    assert!(feed.offset > before || feed.selected > 0);
}

#[tokio::test]
async fn mouse_sidebar_click_navigates() {
    let (mut app, _provider, _posts) = app_with_posts(140, 40).await;
    let _terminal = draw(&mut app, 140, 40);
    let rect = app
        .hit_map
        .iter()
        .find(|area| {
            area.target == reddtty::app::hit::HitTarget::Sidebar(FeedSpec::Subreddit("rust".into()))
        })
        .map(|area| area.rect)
        .expect("r/rust sidebar entry");
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x + 2,
        row: rect.y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert!(matches!(
        app.current(),
        Some(View::Feed(feed)) if feed.spec == FeedSpec::Subreddit("rust".into())
    ));
}

#[tokio::test]
async fn mouse_sort_tab_changes_sort() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    let _terminal = draw(&mut app, 120, 36);
    let rect = app
        .hit_map
        .iter()
        .find(|area| area.target == reddtty::app::hit::HitTarget::FeedSort(FeedSort::New))
        .map(|area| area.rect)
        .expect("New sort tab");
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x + 1,
        row: rect.y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.current_feed().unwrap().sort, FeedSort::New);
}

#[tokio::test]
async fn mouse_click_outside_modal_closes_it() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    app.open_help();
    let _terminal = draw(&mut app, 100, 30);
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert!(app.modal.is_none());
}

#[tokio::test]
async fn mouse_double_click_comment_collapses() {
    let (provider, posts) = demo().await;
    let post = posts.iter().find(|p| p.id == "demo001").cloned().unwrap();
    let comments = provider
        .get_comments(&post.id, CommentSort::Best)
        .await
        .unwrap();
    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        provider,
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    let mut view = PostView::new(ViewId(600), post, CommentSort::Best);
    view.comments = comments;
    view.comments_loading = false;
    view.comments_dirty = true;
    app.nav.push(View::Post(view));
    app.focus = Focus::Comments;
    app.set_size(120, 40);
    app.prepare();
    let _terminal = draw(&mut app, 120, 40);
    let (index, row_id) = {
        let post = app.current_post().unwrap();
        let index = post
            .rows
            .iter()
            .position(|row| row.has_children && !row.is_more)
            .expect("a comment with replies");
        (index, post.rows[index].id.clone())
    };
    let rect = app
        .hit_map
        .iter()
        .find(|area| area.target == reddtty::app::hit::HitTarget::CommentRow(index))
        .map(|area| area.rect)
        .expect("comment hitbox");
    let (x, y) = (rect.x + 3, rect.y);
    for _ in 0..2 {
        app.handle_mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: x,
            row: y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
    }
    assert!(
        app.current_post().unwrap().collapsed.contains(&row_id),
        "double click should collapse the comment"
    );
}

#[tokio::test]
async fn mouse_gallery_arrows_change_image() {
    let (_provider, posts) = demo().await;
    let post = posts
        .iter()
        .find(|p| p.has_gallery() && p.images().len() > 2)
        .cloned()
        .expect("multi-image gallery");
    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let mut app = App::new(
        Arc::new(DemoProvider::new(&Settings::cache_dir())),
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    app.nav
        .push(View::Gallery(GalleryView::new(ViewId(500), post).unwrap()));
    app.set_size(100, 30);
    app.prepare();
    let _terminal = draw(&mut app, 100, 30);
    let rect = app
        .hit_map
        .iter()
        .find(|area| area.target == reddtty::app::hit::HitTarget::GalleryNext)
        .map(|area| area.rect)
        .expect("gallery next hitbox");
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x + 1,
        row: rect.y + 1,
        modifiers: crossterm::event::KeyModifiers::NONE,
    });
    assert_eq!(app.current_gallery().unwrap().index, 1);
}

#[tokio::test]
async fn keyboard_goto_subreddit_opens_feed() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    app.open_goto_modal();
    for c in "r/rust".chars() {
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(matches!(
        app.current(),
        Some(View::Feed(feed)) if feed.spec == FeedSpec::Subreddit("rust".into())
    ));
    assert!(app.modal.is_none());
}

#[tokio::test]
async fn keyboard_search_opens_search_feed() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    app.open_search_modal();
    for c in "terminal".chars() {
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(matches!(
        app.current(),
        Some(View::Feed(feed)) if matches!(feed.spec, FeedSpec::Search { .. })
    ));
}

#[tokio::test]
async fn top_range_switches_sort_to_top() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    app.open_range_modal();
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('k'),
        crossterm::event::KeyModifiers::NONE,
    ));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    let feed = app.current_feed().unwrap();
    assert_eq!(feed.sort, FeedSort::Top);
    assert_eq!(feed.top_range, TopRange::Day);
}

#[tokio::test]
async fn sort_modal_applies_selected_sort() {
    let (mut app, _provider, _posts) = app_with_posts(120, 36).await;
    app.open_sort_modal();
    for _ in 0..2 {
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(app.current_feed().unwrap().sort, FeedSort::New);
}

#[tokio::test]
async fn q_marks_app_for_quit() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('q'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(app.should_quit);
}

#[tokio::test]
async fn config_default_sort_is_honoured() {
    let (provider, _posts) = demo().await;
    let mut settings = Settings::default();
    settings.images.enabled = false;
    settings.general.default_sort = "new".into();
    let (tx, _rx) = unbounded_channel();
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let app = App::new(
        provider,
        settings,
        test_config_path(),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    assert_eq!(app.current_feed().unwrap().sort, FeedSort::New);
}

#[tokio::test]
async fn status_bar_never_clips_quit_hint_at_common_widths() {
    for (w, h) in [
        (30u16, 10u16),
        (60u16, 20u16),
        (80, 24),
        (100, 30),
        (120, 36),
    ] {
        let (mut app, _provider, _posts) = app_with_posts(w, h).await;
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        let last = text.lines().last().unwrap_or("");
        assert!(
            last.contains("Quit"),
            "status bar at {w}x{h} lost the quit hint: {last:?}"
        );
        assert!(
            !last.ends_with('…'),
            "status bar at {w}x{h} ended in a truncation marker: {last:?}"
        );
    }
}

#[tokio::test]
async fn clipped_post_card_never_paints_over_bottom_border() {
    use ratatui::layout::Rect;
    use reddtty::models::PostContent;
    use reddtty::ui::theme::Theme;
    use reddtty::ui::widgets::post_card;

    let (tx, _rx) = unbounded_channel();
    let settings = settings();
    let mut images = ImageManager::new(Picker::halfblocks(), &settings, tx);
    let post = Post {
        id: "clip".into(),
        subreddit: "rust".into(),
        author: "alice".into(),
        title: "A title that would be drawn in a clipped card".into(),
        body: "Body text that must not overwrite the card bottom border. ".repeat(4),
        created_utc: 0,
        score: 10,
        num_comments: 2,
        permalink: "/r/rust/comments/clip".into(),
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
    };
    let area = Rect::new(0, 0, 60, 6);
    let backend = TestBackend::new(60, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            post_card::render(
                frame,
                area,
                &post,
                false,
                &Theme::dark(),
                &settings,
                &mut images,
                true,
                false,
                0,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let bottom: String = (0..60).map(|x| buffer[(x, 5)].symbol()).collect();
    assert!(
        bottom.chars().all(|c| matches!(c, '─' | '╰' | '╯')),
        "clipped card bottom border was overwritten: {bottom:?}"
    );
}

#[tokio::test]
async fn narrow_feed_keeps_post_count_and_more_hint() {
    for (w, h) in [(60u16, 20u16), (80, 24)] {
        let (mut app, _provider, posts) = app_with_posts(w, h).await;
        if let Some(feed) = app.current_feed_mut() {
            feed.has_more = true;
            feed.posts = posts;
            feed.heights_dirty = true;
        }
        app.prepare();
        let terminal = draw(&mut app, w, h);
        let text = screen(&terminal);
        assert!(
            text.contains(&format!(
                "{} posts",
                app.current_feed().unwrap().posts.len()
            )),
            "missing post count at {w}x{h}"
        );
        assert!(text.contains("more"), "missing more hint at {w}x{h}");
    }
}

#[tokio::test]
async fn tiny_layout_keeps_demo_badge_and_admits_hidden_help() {
    let (mut app, _provider, _posts) = app_with_posts(30, 10).await;
    let terminal = draw(&mut app, 30, 10);
    let text = screen(&terminal);
    assert!(
        text.contains("DEMO"),
        "demo mode must stay labelled: {text}"
    );

    app.open_help();
    let terminal = draw(&mut app, 30, 10);
    let text = screen(&terminal);
    assert!(text.contains("Keyboard Shortcuts"));
    assert!(
        text.contains("more"),
        "clipped help must say that shortcuts are hidden: {text}"
    );
    assert!(
        text.contains("Esc"),
        "help must still show how to close: {text}"
    );
}

#[tokio::test]
async fn favorite_removal_confirmation_is_rendered() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    app.settings.favorites.push("rust".into());
    app.focus = reddtty::app::Focus::Sidebar;
    let index = app
        .sidebar_entries()
        .iter()
        .position(|entry| entry.label == "r/rust" && entry.favorite)
        .expect("default rust favorite");
    app.sidebar.selected = index;
    app.toggle_favorite();
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("Remove favorite"), "{text}");
    assert!(text.contains("Remove r/rust from favorites?"), "{text}");
    assert!(text.contains("cancel"), "{text}");
    assert!(app.settings.is_favorite("rust"));
}

#[tokio::test]
async fn top_bar_shows_html_provider_and_error_status() {
    let (mut app, _provider, _posts) = app_with_posts(120, 30).await;
    app.provider_label = "OLD REDDIT".into();
    app.provider_status = Some(reddtty::providers::ProviderStatus::AccessDenied);
    let terminal = draw(&mut app, 120, 30);
    let text = screen(&terminal);
    assert!(text.contains("OLD REDDIT"), "{text}");
    assert!(text.contains("Access denied"), "{text}");
    assert!(!text.contains("DEMO"), "{text}");

    app.provider_status = Some(reddtty::providers::ProviderStatus::HtmlStructureChanged);
    let terminal = draw(&mut app, 120, 30);
    let text = screen(&terminal);
    assert!(text.contains("HTML changed"), "{text}");
}

#[tokio::test]
async fn narrow_top_bar_keeps_provider_badge_over_help() {
    let (mut app, _provider, _posts) = app_with_posts(60, 20).await;
    app.provider_label = "REDLIB · redlib.hbubli.cc".into();
    app.provider_status = Some(reddtty::providers::ProviderStatus::LoginRequired);
    let terminal = draw(&mut app, 60, 20);
    let text = screen(&terminal);
    assert!(text.contains("REDLIB"), "{text}");
    // Status is preferred over the help affordance when both cannot fit.
    assert!(text.contains("Login required"), "{text}");
}

#[tokio::test]
async fn long_provider_error_wraps_instead_of_truncating_feedback() {
    let (mut app, _provider, _posts) = app_with_posts(130, 32).await;
    if let Some(feed) = app.current_feed_mut() {
        feed.posts.clear();
        feed.loading_initial = false;
        feed.error = Some(
            "New is not available for Home from REDLIB right now. The source's Reddit backend refused this listing (access denied or CAPTCHA). Other sorts such as Hot may still work. Try another sort or a different permitted source; ReddTTY does not bypass access controls or switch instances automatically."
                .into(),
        );
    }
    app.prepare();
    let terminal = draw(&mut app, 130, 32);
    let text = screen(&terminal);
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(text.contains("Unable to load posts."));
    assert!(
        flat.contains("sorts such as Hot may still work"),
        "wrapped error feedback is missing: {text}"
    );
    assert!(
        flat.contains("does not bypass access controls"),
        "actionable part of the error is missing: {text}"
    );
}

#[tokio::test]
async fn settings_modal_shows_change_config_and_theme() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('S'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("Settings"), "{text}");
    assert!(text.contains("Change config"), "{text}");
    assert!(text.contains("Theme:"), "{text}");

    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(text.contains("Choose config"), "{text}");
    assert!(text.contains("Demo (offline)"), "{text}");
}

#[tokio::test]
async fn empty_favorites_section_is_visible_with_add_hint() {
    let (mut app, _provider, _posts) = app_with_posts(100, 30).await;
    assert!(app.settings.favorites.is_empty());
    let terminal = draw(&mut app, 100, 30);
    let text = screen(&terminal);
    assert!(
        text.contains("FAVORITES"),
        "missing favorites heading: {text}"
    );
    assert!(text.contains("f to add"), "missing add hint: {text}");
}
