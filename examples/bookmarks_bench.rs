//! Bookmark/read-history persistence benchmark used by release verification.
//! Run with: cargo build --release --locked --example bookmarks_bench

use std::time::Instant;

use reddtty::app::modal::BookmarksModal;
use reddtty::library::{Library, SavedPost};
use reddtty::models::{Post, PostContent};

fn post(index: usize) -> Post {
    Post {
        id: format!("bench{index}"),
        subreddit: ["rust", "linux", "programming"][index % 3].into(),
        author: format!("author{index}"),
        title: format!("Bookmark {index}: terminal reading and long-term storage"),
        body: "A short body for the benchmark.".into(),
        created_utc: 1_700_000_000 - index as i64,
        score: index as i64,
        num_comments: (index % 50) as u32,
        permalink: format!("/r/rust/comments/bench{index}"),
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

fn run(count: usize) {
    let dir = std::env::temp_dir().join(format!(
        "reddtty-bookmark-bench-{}-{count}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.toml");
    let mut library = Library::default();
    for index in 0..count {
        library.bookmarks.push(SavedPost {
            post: post(index),
            saved_at: 1_700_000_000 + index as i64,
        });
    }
    let start = Instant::now();
    library.save(&path).unwrap();
    let save = start.elapsed();
    let start = Instant::now();
    let (loaded, warnings) = Library::load(&path);
    let load = start.elapsed();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(loaded.bookmarks.len(), count);
    let modal = BookmarksModal {
        query: "bookmark 999".into(),
        ..BookmarksModal::default()
    };
    let start = Instant::now();
    let matches = modal.filtered(&loaded).len();
    let search = start.elapsed();
    println!(
        "bookmarks={count} save_ms={:.1} load_ms={:.1} search_ms={:.2} matches={matches} bytes={}",
        save.as_secs_f64() * 1000.0,
        load.as_secs_f64() * 1000.0,
        search.as_secs_f64() * 1000.0,
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    );
    let _ = std::fs::remove_dir_all(dir);
}

fn main() {
    for count in [1_000usize, 10_000] {
        run(count);
    }
    let mut library = Library::default();
    let start = Instant::now();
    for index in 0..50_000 {
        library.mark_comment_read(&format!("comment{index}"));
    }
    let start_save = Instant::now();
    let dir = std::env::temp_dir().join(format!("reddtty-read-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.toml");
    library.save(&path).unwrap();
    let save = start_save.elapsed();
    let start_load = Instant::now();
    let (loaded, _) = Library::load(&path);
    let load = start_load.elapsed();
    println!(
        "read_history=50000 mutate_ms={:.1} save_ms={:.1} load_ms={:.1} bytes={}",
        start.elapsed().as_secs_f64() * 1000.0,
        save.as_secs_f64() * 1000.0,
        load.as_secs_f64() * 1000.0,
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    );
    assert_eq!(loaded.read_comments.len(), 50_000);
    let _ = std::fs::remove_dir_all(dir);
}
