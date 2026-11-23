//! Synthetic stress harness for ReddTTY.
//!
//! This is intentionally an example (not part of the normal test suite) so it
//! can be built and timed with `/usr/bin/time -l` without slowing CI.
//!
//! Run:
//!   cargo build --release --example stress
//!   /usr/bin/time -l target/release/examples/stress
//!
//! It renders 500 posts, a 1000-node nested comment tree and encodes 100 real
//! local images through the same ImageManager used by the application.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use reddtty::app::post::PostView;
use reddtty::app::{App, Focus, View};
use reddtty::config::Settings;
use reddtty::event::{AppEvent, ViewId};
use reddtty::media::ImageManager;
use reddtty::models::{Comment, CommentSort, Post, PostContent};
use reddtty::providers::demo::DemoProvider;
use reddtty::ui;
use tokio::sync::mpsc::unbounded_channel;

const W: u16 = 160;
const H: u16 = 45;
const POSTS: usize = 500;
const COMMENT_NODES: usize = 1000;
const IMAGES: usize = 100;
const FRAMES: usize = 1200;

fn post(index: usize) -> Post {
    let kind = index % 12;
    let content = match kind {
        0 | 6 => PostContent::Text,
        1 | 7 => PostContent::Image {
            url: format!("demo://img/aurora.png?{index}"),
        },
        2 => PostContent::Gallery {
            items: (0..5)
                .map(|i| reddtty::models::MediaItem::new(format!("demo://img/{i}.png")))
                .collect(),
        },
        3 => PostContent::Link {
            url: format!("https://example.com/stress/{index}"),
            thumbnail: Some("demo://img/city.png".into()),
        },
        4 => PostContent::Video {
            url: format!("https://example.com/video/{index}"),
            thumbnail: Some("demo://img/terminal.png".into()),
            duration: Some(120),
        },
        5 => PostContent::Removed,
        _ => {
            if index.is_multiple_of(17) {
                PostContent::Text
            } else {
                PostContent::Image {
                    url: format!("demo://img/mountains.png?{index}"),
                }
            }
        }
    };
    Post {
        id: format!("stress{index}"),
        subreddit: ["rust", "linux", "programming", "technology", "unixporn"][index % 5].into(),
        author: format!("author{index}"),
        title: format!(
            "Stress post {index}: a reasonably long title about terminal interfaces and scrolling"
        ),
        body: "This is a synthetic body used for layout and wrapping stress. ".repeat(6),
        created_utc: 1_700_000_000 - (index as i64 * 37),
        score: (index as i64 * 13) % 5000,
        num_comments: (index * 7) as u32,
        permalink: format!("/r/stress/comments/stress{index}/title/"),
        content,
        flair: Some("Stress".into()),
        nsfw: index.is_multiple_of(43),
        spoiler: index.is_multiple_of(59),
        stickied: false,
        locked: false,
        removed: false,
        is_self: true,
        domain: None,
        thumbnail: None,
    }
}

/// 100 roots, each with a chain of 9 replies = 1000 nodes.
fn nested_comments() -> Vec<Comment> {
    const ROOTS: usize = 100;
    const PER_ROOT: usize = 10;
    let mut roots = Vec::with_capacity(ROOTS);
    for r in 0..ROOTS {
        let mut root = Comment::new(
            format!("c{r}_0"),
            format!("author{r}"),
            format!("Root comment {r} with a body long enough to wrap in a narrow column. "),
        );
        root.score = 1000 - r as i64;
        root.created_utc = 1_700_000_000 - r as i64 * 60;
        let root_created = root.created_utc;
        let mut parent = &mut root;
        for d in 1..PER_ROOT {
            let mut child = Comment::new(
                format!("c{r}_{d}"),
                format!("author{r}_{d}"),
                format!(
                    "Nested reply depth {d} for root {r}. This text checks wrapping and the tree rail. "
                ),
            );
            child.score = (100 - d as i64).max(1);
            child.created_utc = root_created + d as i64 * 10;
            parent.replies.push(child);
            parent = parent.replies.last_mut().unwrap();
        }
        roots.push(root);
    }
    assert_eq!(roots.len() * PER_ROOT, COMMENT_NODES);
    roots
}

fn percentiles(mut values: Vec<Duration>) -> (f64, f64, f64) {
    values.sort();
    let avg = values.iter().map(|d| d.as_secs_f64()).sum::<f64>() / values.len() as f64;
    let p95 = values[(values.len() * 95 / 100).min(values.len() - 1)].as_secs_f64();
    let max = values.last().copied().unwrap_or_default().as_secs_f64();
    (avg * 1000.0, p95 * 1000.0, max * 1000.0)
}

fn app_with_feed(posts: Vec<Post>) -> App {
    let (tx, _rx) = unbounded_channel();
    let mut settings = Settings::default();
    settings.images.enabled = false;
    settings.general.show_sidebar = true;
    settings.ui.show_right_panel = true;
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let provider = Arc::new(DemoProvider::new(&Settings::cache_dir()));
    let mut app = App::new(
        provider,
        settings,
        std::env::temp_dir().join("reddtty-stress-config.toml"),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    if let Some(feed) = app.current_feed_mut() {
        feed.posts = posts;
        feed.loading_initial = false;
        feed.has_more = false;
        feed.heights_dirty = true;
    }
    app.set_size(W, H);
    app
}

fn app_with_comments(post: Post, comments: Vec<Comment>) -> App {
    let (tx, _rx) = unbounded_channel();
    let mut settings = Settings::default();
    settings.images.enabled = false;
    let images = ImageManager::new(Picker::halfblocks(), &settings, tx.clone());
    let provider = Arc::new(DemoProvider::new(&Settings::cache_dir()));
    let mut app = App::new(
        provider,
        settings,
        std::env::temp_dir().join("reddtty-stress-config.toml"),
        images,
        tx,
        None,
        None,
        Vec::new(),
    );
    let mut view = PostView::new(ViewId(900_001), post, CommentSort::Best);
    view.comments = comments;
    view.comments_loading = false;
    view.comments_dirty = true;
    app.nav.push(View::Post(view));
    app.focus = Focus::Comments;
    app.set_size(W, H);
    app
}

fn draw_once(app: &mut App, terminal: &mut Terminal<TestBackend>) -> Duration {
    let start = Instant::now();
    app.prepare();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    start.elapsed()
}

fn feed_bench() {
    let mut app = app_with_feed((0..POSTS).map(post).collect());
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    let first = draw_once(&mut app, &mut terminal);
    let mut frame_times = Vec::with_capacity(FRAMES);
    let mut input_times = Vec::with_capacity(FRAMES);
    for i in 0..FRAMES {
        let key = if i % 2 == 0 {
            crossterm::event::KeyCode::Char('j')
        } else {
            crossterm::event::KeyCode::Char('k')
        };
        let start = Instant::now();
        app.handle_key(crossterm::event::KeyEvent::new(
            key,
            crossterm::event::KeyModifiers::NONE,
        ));
        let input_elapsed = start.elapsed();
        input_times.push(input_elapsed);
        app.prepare();
        let render_start = Instant::now();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        frame_times.push(render_start.elapsed());
    }
    let (avg, p95, max) = percentiles(frame_times);
    let (key_avg, key_p95, key_max) = percentiles(input_times);
    println!(
        "feed posts={POSTS} first_frame_ms={:.3} frame_avg_ms={avg:.3} frame_p95_ms={p95:.3} frame_max_ms={max:.3} key_avg_ms={key_avg:.4} key_p95_ms={key_p95:.4} key_max_ms={key_max:.4}",
        first.as_secs_f64() * 1000.0
    );
}

fn comments_bench() {
    let post = post(0);
    let comments = nested_comments();
    let mut app = app_with_comments(post, comments);
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    let first = draw_once(&mut app, &mut terminal);
    let mut frame_times = Vec::with_capacity(400);
    for i in 0..400 {
        let key = if i % 2 == 0 {
            crossterm::event::KeyCode::Char('j')
        } else {
            crossterm::event::KeyCode::Char('k')
        };
        app.handle_key(crossterm::event::KeyEvent::new(
            key,
            crossterm::event::KeyModifiers::NONE,
        ));
        app.prepare();
        let start = Instant::now();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        frame_times.push(start.elapsed());
    }
    // Collapse/expand the selected root repeatedly (each rebuild re-flattens
    // the 1000-node tree).
    let mut toggle_times = Vec::with_capacity(60);
    for _ in 0..60 {
        let start = Instant::now();
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::NONE,
        ));
        app.prepare();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();
        toggle_times.push(start.elapsed());
    }
    let (avg, p95, max) = percentiles(frame_times);
    let (tavg, tp95, tmax) = percentiles(toggle_times);
    println!(
        "comments nodes={COMMENT_NODES} first_frame_ms={:.3} frame_avg_ms={avg:.3} frame_p95_ms={p95:.3} frame_max_ms={max:.3} collapse_avg_ms={tavg:.3} collapse_p95_ms={tp95:.3} collapse_max_ms={tmax:.3}",
        first.as_secs_f64() * 1000.0
    );
}

async fn images_bench() {
    let mut settings = Settings::default();
    settings.images.enabled = true;
    settings.images.protocol = reddtty::config::ImageProtocolSetting::Halfblocks;
    settings.images.max_cache_mb = 128;
    settings.images.max_download_mb = 10;
    let (tx, mut rx) = unbounded_channel();
    let mut images = ImageManager::new(Picker::halfblocks(), &settings, tx);

    let root = Settings::cache_dir().join(format!("stress-images-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source = include_bytes!("../fixtures/images/aurora.png");
    for i in 0..IMAGES {
        std::fs::write(root.join(format!("{i}.png")), source).unwrap();
    }

    let canvas = (80u16, 12u16);
    let start = Instant::now();
    let mut next = 0usize;
    let mut done = 0usize;
    let mut failures = 0usize;
    while next < IMAGES.min(64) {
        let path = root.join(format!("{next}.png"));
        let url = url::Url::from_file_path(&path).unwrap().to_string();
        images.request(&url, &url, canvas.0, canvas.1);
        next += 1;
    }
    while done < IMAGES {
        let event = tokio::time::timeout(Duration::from_secs(120), rx.recv())
            .await
            .expect("image timeout")
            .expect("image channel");
        if let AppEvent::ImageReady { key, result } = event {
            if result.is_err() {
                failures += 1;
            }
            images.on_result(key, result);
            done += 1;
            if next < IMAGES {
                let path = root.join(format!("{next}.png"));
                let url = url::Url::from_file_path(&path).unwrap().to_string();
                images.request(&url, &url, canvas.0, canvas.1);
                next += 1;
            }
        }
    }
    let elapsed = start.elapsed();
    let _ = std::fs::remove_dir_all(&root);
    println!(
        "images count={IMAGES} size={}x{} total_ms={:.1} avg_ms={:.1} failures={failures} cache_len={} cache_bytes={}",
        canvas.0,
        canvas.1,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / IMAGES as f64,
        images.cache_len(),
        images.cache_used_bytes()
    );
}

fn rss_kib() -> Option<u64> {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()
}

#[tokio::main]
async fn main() {
    println!(
        "reddtty stress harness: {POSTS} posts, {COMMENT_NODES} comments, {IMAGES} images, {FRAMES} feed frames"
    );
    println!("rss_before_kib={:?}", rss_kib());
    feed_bench();
    println!("rss_after_feed_kib={:?}", rss_kib());
    comments_bench();
    println!("rss_after_comments_kib={:?}", rss_kib());
    images_bench().await;
    println!("rss_after_images_kib={:?}", rss_kib());
}
