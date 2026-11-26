//! Local, provider-independent user library.
//!
//! Bookmarks, read state and local filters never touch Reddit. They live in a
//! `library.toml` file next to the active config so demo, Redlib, Old Reddit
//! and OAuth sessions all share the same offline-friendly data.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{FeedSort, FeedSpec, Post};
use crate::persistence;

/// A post saved together with the snapshot that was visible when it was saved.
/// The snapshot is what makes the bookmark readable when the source is down.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SavedPost {
    pub post: Post,
    #[serde(default)]
    pub saved_at: i64,
}

/// Local, client-side filters. These are explicit user choices, not Reddit
/// preferences, and never require an account.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LocalFilters {
    pub keywords: Vec<String>,
    pub subreddits: Vec<String>,
    pub authors: Vec<String>,
    pub domains: Vec<String>,
    pub hide_read: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Keyword,
    Subreddit,
    Author,
    Domain,
}

impl FilterKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Subreddit => "subreddit",
            Self::Author => "author",
            Self::Domain => "domain",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterEntry {
    pub kind: FilterKind,
    pub value: String,
}

impl LocalFilters {
    /// Stable row order used by the Filters modal.
    pub fn entries(&self) -> Vec<FilterEntry> {
        let mut out = Vec::new();
        out.extend(self.keywords.iter().map(|value| FilterEntry {
            kind: FilterKind::Keyword,
            value: value.clone(),
        }));
        out.extend(self.subreddits.iter().map(|value| FilterEntry {
            kind: FilterKind::Subreddit,
            value: value.clone(),
        }));
        out.extend(self.authors.iter().map(|value| FilterEntry {
            kind: FilterKind::Author,
            value: value.clone(),
        }));
        out.extend(self.domains.iter().map(|value| FilterEntry {
            kind: FilterKind::Domain,
            value: value.clone(),
        }));
        out
    }

    pub fn remove_entry(&mut self, kind: FilterKind, value: &str) -> bool {
        let list = match kind {
            FilterKind::Keyword => &mut self.keywords,
            FilterKind::Subreddit => &mut self.subreddits,
            FilterKind::Author => &mut self.authors,
            FilterKind::Domain => &mut self.domains,
        };
        let before = list.len();
        list.retain(|item| !item.eq_ignore_ascii_case(value));
        before != list.len()
    }

    pub fn is_active(&self) -> bool {
        self.hide_read
            || !self.keywords.is_empty()
            || !self.subreddits.is_empty()
            || !self.authors.is_empty()
            || !self.domains.is_empty()
    }

    /// Number of individual filter rules, for display.
    pub fn rule_count(&self) -> usize {
        self.keywords.len()
            + self.subreddits.len()
            + self.authors.len()
            + self.domains.len()
            + usize::from(self.hide_read)
    }

    /// Returns the rule that hides the post, if any.
    pub fn matched_rule(&self, post: &Post, read: bool) -> Option<String> {
        if self.hide_read && read {
            return Some("read".into());
        }
        let title = post.title.to_ascii_lowercase();
        let body = post.body.to_ascii_lowercase();
        for keyword in &self.keywords {
            let needle = keyword.trim().to_ascii_lowercase();
            if !needle.is_empty() && (title.contains(&needle) || body.contains(&needle)) {
                return Some(format!("keyword:{keyword}"));
            }
        }
        for subreddit in &self.subreddits {
            if subreddit.eq_ignore_ascii_case(post.subreddit.trim()) {
                return Some(format!("subreddit:{subreddit}"));
            }
        }
        for author in &self.authors {
            if author.eq_ignore_ascii_case(post.author.trim()) {
                return Some(format!("author:{author}"));
            }
        }
        let domain = post.domain_or_subreddit().to_ascii_lowercase();
        for blocked in &self.domains {
            let blocked = blocked.trim().to_ascii_lowercase();
            if !blocked.is_empty()
                && (domain == blocked || domain.ends_with(&format!(".{blocked}")))
            {
                return Some(format!("domain:{blocked}"));
            }
        }
        None
    }

    pub fn hides(&self, post: &Post, read: bool) -> bool {
        self.matched_rule(post, read).is_some()
    }

    /// Add a filter from a compact `kind:value` expression. A bare value is a
    /// keyword. Returns an error message suitable for the input modal.
    pub fn add_expression(&mut self, raw: &str) -> Result<String, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("Type a filter such as keyword:ai or sub:politics.".into());
        }
        let (kind, value) = match raw.split_once(':') {
            Some((kind, value)) => (kind.trim().to_ascii_lowercase(), value.trim()),
            None => ("keyword".to_string(), raw),
        };
        if value.is_empty() {
            return Err("The filter value is empty.".into());
        }
        if value.chars().count() > 80 {
            return Err("The filter value is too long (max 80 characters).".into());
        }
        match kind.as_str() {
            "keyword" | "word" | "k" => {
                push_unique(&mut self.keywords, value);
                Ok(format!("Blocking posts containing “{value}”."))
            }
            "sub" | "subreddit" | "r" => {
                let name = crate::utils::text::normalize_subreddit(value)
                    .ok_or_else(|| "Enter a valid subreddit name.".to_string())?;
                push_unique(&mut self.subreddits, &name);
                Ok(format!("Muting r/{name} locally."))
            }
            "author" | "user" | "u" => {
                let name = value.trim_start_matches("u/");
                if name.is_empty() || name.chars().any(char::is_whitespace) {
                    return Err("Enter a valid Reddit username.".into());
                }
                push_unique(&mut self.authors, name);
                Ok(format!("Muting u/{name} locally."))
            }
            "domain" | "d" => {
                let domain = value
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .trim_end_matches('/')
                    .to_ascii_lowercase();
                if !domain.contains('.') || domain.contains(char::is_whitespace) {
                    return Err("Enter a domain such as example.com.".into());
                }
                push_unique(&mut self.domains, &domain);
                Ok(format!("Muting links to {domain}."))
            }
            _ => Err("Unknown filter kind. Use keyword, sub, author or domain.".into()),
        }
    }
}

fn apply_set_delta(
    target: &mut BTreeSet<String>,
    base: &BTreeSet<String>,
    current: &BTreeSet<String>,
) {
    for removed in base.difference(current) {
        target.remove(removed);
    }
    for added in current.difference(base) {
        target.insert(added.clone());
    }
}

fn apply_vec_delta(target: &mut Vec<String>, base: &[String], current: &[String]) {
    for removed in base.iter().filter(|value| !current.contains(value)) {
        target.retain(|value| value != removed);
    }
    for added in current.iter().filter(|value| !base.contains(value)) {
        if !target.contains(added) {
            target.push(added.clone());
        }
    }
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(value))
    {
        list.push(value.to_string());
    }
}

/// Every locally stored item. The file is intentionally plain TOML so users can
/// inspect and back it up with the config.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub bookmarks: Vec<SavedPost>,
    pub read_posts: BTreeSet<String>,
    pub read_comments: BTreeSet<String>,
    pub filters: LocalFilters,
    /// Last user-chosen feed/sort, restored on the next launch.
    pub last_feed: Option<FeedSpec>,
    pub last_sort: Option<FeedSort>,
    #[serde(default)]
    pub last_top_range: Option<crate::models::TopRange>,
    #[serde(default)]
    pub last_selected_post: Option<String>,
    /// Snapshot of the last successfully loaded/saved state. Used to merge
    /// changes made by another ReddTTY process instead of overwriting them.
    #[serde(skip)]
    pub baseline: Option<Box<Library>>,
}

pub struct LibraryLoad {
    pub library: Library,
    pub warnings: Vec<String>,
    pub corrupt: bool,
    pub recovery: Option<PathBuf>,
}

impl Library {
    /// `library.toml` lives next to the active config file.
    pub fn path_for(config_path: &Path) -> PathBuf {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("library.toml")
    }

    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let loaded = Self::load_checked(path);
        (loaded.library, loaded.warnings)
    }

    pub fn load_checked(path: &Path) -> LibraryLoad {
        let mut warnings = Vec::new();
        let text = match persistence::read_text(path) {
            Ok(Some(text)) => text,
            Ok(None) => {
                let mut library = Self::default();
                library.set_baseline();
                return LibraryLoad {
                    library,
                    warnings,
                    corrupt: false,
                    recovery: None,
                };
            }
            Err(error) => {
                let recovery = persistence::backup_corrupt(path).ok();
                warnings.push(format!(
                    "{error}. Bookmarks start empty; saving is disabled until the file is repaired.{}{}",
                    recovery
                        .as_ref()
                        .map(|path| format!(" Original bytes preserved at {}.", path.display()))
                        .unwrap_or_default(),
                    if recovery.is_none() {
                        " A recovery backup could not be created."
                    } else {
                        ""
                    }
                ));
                let mut library = Self::default();
                library.set_baseline();
                return LibraryLoad {
                    library,
                    warnings,
                    corrupt: true,
                    recovery,
                };
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(mut library) => {
                library.normalize();
                let unknown = persistence::unknown_top_level_keys(
                    &text,
                    &[
                        "bookmarks",
                        "read_posts",
                        "read_comments",
                        "filters",
                        "last_feed",
                        "last_sort",
                        "last_top_range",
                        "last_selected_post",
                    ],
                );
                if !unknown.is_empty() {
                    warnings.push(format!(
                        "Library contains unknown top-level key(s): {}. Check for typos; the next save will drop them.",
                        unknown.join(", ")
                    ));
                }
                library.set_baseline();
                LibraryLoad {
                    library,
                    warnings,
                    corrupt: false,
                    recovery: None,
                }
            }
            Err(error) => {
                let recovery = match persistence::backup_corrupt(path) {
                    Ok(backup) => {
                        warnings.push(format!(
                            "Invalid library file ({}): {}. A recovery copy was written to {}. Saving is disabled until the original is repaired or removed.",
                            path.display(),
                            error.to_string().lines().next().unwrap_or("parse error"),
                            backup.display()
                        ));
                        Some(backup)
                    }
                    Err(backup_error) => {
                        warnings.push(format!(
                            "Invalid library file ({}): {}. Recovery backup failed: {backup_error}. Saving is disabled.",
                            path.display(),
                            error.to_string().lines().next().unwrap_or("parse error")
                        ));
                        None
                    }
                };
                LibraryLoad {
                    library: Self::default(),
                    warnings,
                    corrupt: true,
                    recovery,
                }
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self)
            .map_err(|error| format!("Could not serialise library: {error}"))?;
        persistence::write_private(path, text.as_bytes()).map_err(|error| error.to_string())
    }

    fn set_baseline(&mut self) {
        let mut snapshot = self.clone();
        snapshot.baseline = None;
        self.baseline = Some(Box::new(snapshot));
    }

    /// Merge this process's changes with the current on-disk library under an
    /// inter-process lock. Bookmark/read/filter additions and removals are
    /// applied as deltas against `baseline`, so two sessions cannot silently
    /// delete each other's changes. Last-view fields remain last-writer-wins.
    pub fn save_merged(&mut self, path: &Path) -> Result<(), String> {
        let _lock = persistence::FileLock::acquire(path, std::time::Duration::from_secs(2))
            .map_err(|error| error.to_string())?;
        let disk = match persistence::read_text(path).map_err(|error| error.to_string())? {
            Some(text) => match toml::from_str::<Library>(&text) {
                Ok(mut library) => {
                    library.normalize();
                    library
                }
                Err(error) => {
                    return Err(format!(
                        "Another process wrote an invalid library file; refusing to overwrite it: {}",
                        error.to_string().lines().next().unwrap_or("parse error")
                    ));
                }
            },
            None => Library::default(),
        };
        let merged = self.merge_with(disk);
        let text = toml::to_string_pretty(&merged)
            .map_err(|error| format!("Could not serialise library: {error}"))?;
        persistence::write_private(path, text.as_bytes()).map_err(|error| error.to_string())?;
        *self = merged;
        self.set_baseline();
        Ok(())
    }

    fn merge_with(&self, disk: Library) -> Library {
        let Some(base) = self.baseline.as_deref() else {
            return self.clone();
        };
        let mut merged = disk;

        let base_ids: BTreeSet<String> = base.bookmarks.iter().map(|s| s.post.id.clone()).collect();
        let self_ids: BTreeSet<String> = self.bookmarks.iter().map(|s| s.post.id.clone()).collect();
        merged.bookmarks.retain(|saved| {
            !base_ids.contains(&saved.post.id) || self_ids.contains(&saved.post.id)
        });
        for saved in &self.bookmarks {
            if !base_ids.contains(&saved.post.id) {
                if let Some(existing) = merged
                    .bookmarks
                    .iter_mut()
                    .find(|item| item.post.id == saved.post.id)
                {
                    *existing = saved.clone();
                } else {
                    merged.bookmarks.push(saved.clone());
                }
            }
        }

        apply_set_delta(&mut merged.read_posts, &base.read_posts, &self.read_posts);
        apply_set_delta(
            &mut merged.read_comments,
            &base.read_comments,
            &self.read_comments,
        );
        apply_vec_delta(
            &mut merged.filters.keywords,
            &base.filters.keywords,
            &self.filters.keywords,
        );
        apply_vec_delta(
            &mut merged.filters.subreddits,
            &base.filters.subreddits,
            &self.filters.subreddits,
        );
        apply_vec_delta(
            &mut merged.filters.authors,
            &base.filters.authors,
            &self.filters.authors,
        );
        apply_vec_delta(
            &mut merged.filters.domains,
            &base.filters.domains,
            &self.filters.domains,
        );
        if self.filters.hide_read != base.filters.hide_read {
            merged.filters.hide_read = self.filters.hide_read;
        }
        merged.last_feed = self.last_feed.clone();
        merged.last_sort = self.last_sort;
        merged.last_top_range = self.last_top_range;
        merged.last_selected_post = self.last_selected_post.clone();
        merged.normalize();
        merged
    }

    pub fn normalize(&mut self) {
        let mut seen = BTreeSet::new();
        self.bookmarks
            .retain(|saved| !saved.post.id.is_empty() && seen.insert(saved.post.id.clone()));
        self.bookmarks
            .sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        self.filters
            .subreddits
            .retain(|name| crate::utils::text::normalize_subreddit(name).is_some());
        self.filters.subreddits.dedup();
        self.filters.authors.dedup();
        self.filters.keywords.dedup();
        self.filters.domains.dedup();
    }

    pub fn is_bookmarked(&self, post_id: &str) -> bool {
        self.bookmarks.iter().any(|saved| saved.post.id == post_id)
    }

    /// Adds or removes a snapshot bookmark and returns the new state.
    pub fn toggle_bookmark(&mut self, post: &Post) -> bool {
        if self.is_bookmarked(&post.id) {
            self.remove_bookmark(&post.id);
            false
        } else {
            let saved_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs() as i64)
                .unwrap_or(0);
            self.bookmarks.insert(
                0,
                SavedPost {
                    post: post.clone(),
                    saved_at,
                },
            );
            true
        }
    }

    pub fn remove_bookmark(&mut self, post_id: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|saved| saved.post.id != post_id);
        before != self.bookmarks.len()
    }

    pub fn saved_post(&self, index: usize) -> Option<&Post> {
        self.bookmarks.get(index).map(|saved| &saved.post)
    }

    pub fn is_post_read(&self, post_id: &str) -> bool {
        self.read_posts.contains(post_id)
    }

    /// Returns true when the post became read.
    pub fn mark_post_read(&mut self, post_id: &str) -> bool {
        self.read_posts.insert(post_id.to_string())
    }

    pub fn mark_post_unread(&mut self, post_id: &str) -> bool {
        self.read_posts.remove(post_id)
    }

    /// Returns true when the post is read after toggling.
    pub fn toggle_post_read(&mut self, post_id: &str) -> bool {
        if self.is_post_read(post_id) {
            self.mark_post_unread(post_id);
            false
        } else {
            self.mark_post_read(post_id);
            true
        }
    }

    pub fn is_comment_read(&self, comment_id: &str) -> bool {
        self.read_comments.contains(comment_id)
    }

    pub fn mark_comment_read(&mut self, comment_id: &str) -> bool {
        self.read_comments.insert(comment_id.to_string())
    }

    pub fn clear_read(&mut self) {
        self.read_posts.clear();
        self.read_comments.clear();
    }

    pub fn clear_filters(&mut self) {
        self.filters = LocalFilters::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FeedSort, FeedSpec, Post, PostContent, SearchScope};
    use std::fs;

    fn post(id: &str) -> Post {
        Post {
            id: id.into(),
            subreddit: "rust".into(),
            author: "alice".into(),
            title: "Learning Rust".into(),
            body: "A post about memory safety".into(),
            created_utc: 0,
            score: 1,
            num_comments: 0,
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

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "reddtty-library-{tag}-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn bookmark_snapshot_roundtrips_through_disk() {
        let path = temp_path("roundtrip");
        let mut library = Library::default();
        let mut saved = post("p1");
        saved.body = "snapshot body".into();
        assert!(library.toggle_bookmark(&saved));
        library.mark_post_read("p1");
        library.filters.add_expression("keyword:memory").unwrap();
        library.save(&path).unwrap();

        let (loaded, warnings) = Library::load(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.bookmarks.len(), 1);
        assert_eq!(loaded.bookmarks[0].post.body, "snapshot body");
        assert!(loaded.is_post_read("p1"));
        assert_eq!(loaded.filters.keywords, vec!["memory"]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn non_utf8_library_is_backed_up_and_stays_read_only() {
        let path = temp_path("nonutf8");
        let bytes = [0xff, 0xfe, 0x00, 0x80];
        fs::write(&path, bytes).unwrap();
        let loaded = Library::load_checked(&path);
        assert!(loaded.corrupt);
        let recovery = loaded.recovery.expect("backup");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&recovery).unwrap(), bytes);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&recovery);
    }

    #[test]
    fn two_stale_sessions_merge_added_bookmarks() {
        let path = temp_path("merge-add");
        Library::default().save(&path).unwrap();
        let mut first = Library::load(&path).0;
        let mut second = Library::load(&path).0;
        first.toggle_bookmark(&post("first"));
        first.save_merged(&path).unwrap();
        second.toggle_bookmark(&post("second"));
        second.save_merged(&path).unwrap();
        let (disk, _) = Library::load(&path);
        assert!(disk.is_bookmarked("first"));
        assert!(disk.is_bookmarked("second"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn removal_in_one_session_does_not_resurrect_a_bookmark_from_another() {
        let path = temp_path("merge-remove");
        let mut seed = Library::default();
        seed.toggle_bookmark(&post("keep"));
        seed.toggle_bookmark(&post("remove"));
        seed.save(&path).unwrap();
        let mut remover = Library::load(&path).0;
        let mut adder = Library::load(&path).0;
        remover.remove_bookmark("remove");
        remover.save_merged(&path).unwrap();
        adder.toggle_bookmark(&post("new"));
        adder.save_merged(&path).unwrap();
        let (disk, _) = Library::load(&path);
        assert!(disk.is_bookmarked("keep"));
        assert!(disk.is_bookmarked("new"));
        assert!(!disk.is_bookmarked("remove"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn read_state_and_filters_merge_without_lost_updates() {
        let path = temp_path("merge-state");
        Library::default().save(&path).unwrap();
        let mut first = Library::load(&path).0;
        let mut second = Library::load(&path).0;
        first.mark_post_read("p1");
        first.filters.keywords.push("ai".into());
        first.save_merged(&path).unwrap();
        second.mark_post_read("p2");
        second.filters.authors.push("spam".into());
        second.save_merged(&path).unwrap();
        let (disk, _) = Library::load(&path);
        assert!(disk.is_post_read("p1"));
        assert!(disk.is_post_read("p2"));
        assert_eq!(disk.filters.keywords, vec!["ai"]);
        assert_eq!(disk.filters.authors, vec!["spam"]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn bookmarks_are_not_truncated_at_five_hundred() {
        let path = temp_path("bookmark-limit");
        let mut library = Library::default();
        for index in 0..1001 {
            library.toggle_bookmark(&post(&format!("b{index}")));
        }
        assert_eq!(library.bookmarks.len(), 1001);
        library.save_merged(&path).unwrap();
        let (disk, _) = Library::load(&path);
        assert_eq!(disk.bookmarks.len(), 1001);
        assert!(disk.is_bookmarked("b0"));
        assert!(disk.is_bookmarked("b1000"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn last_feed_and_sort_roundtrip() {
        let path = temp_path("last-view");
        let library = Library {
            last_feed: Some(FeedSpec::Search {
                query: "rust async".into(),
                scope: SearchScope::Subreddit("rust".into()),
            }),
            last_sort: Some(FeedSort::New),
            last_top_range: Some(crate::models::TopRange::All),
            last_selected_post: Some("abc123".into()),
            ..Library::default()
        };
        library.save(&path).unwrap();
        let (loaded, warnings) = Library::load(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.last_feed, library.last_feed);
        assert_eq!(loaded.last_sort, Some(FeedSort::New));
        assert_eq!(loaded.last_top_range, Some(crate::models::TopRange::All));
        assert_eq!(loaded.last_selected_post.as_deref(), Some("abc123"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn toggling_a_bookmark_removes_it() {
        let mut library = Library::default();
        assert!(library.toggle_bookmark(&post("p")));
        assert!(library.is_bookmarked("p"));
        assert!(!library.toggle_bookmark(&post("p")));
        assert!(!library.is_bookmarked("p"));
    }

    #[test]
    fn local_filters_match_keywords_subs_authors_domains_and_read_state() {
        let mut filters = LocalFilters::default();
        assert!(filters.add_expression("keyword:memory").is_ok());
        assert!(filters.add_expression("sub:rust").is_ok());
        assert!(filters.add_expression("author:alice").is_ok());
        assert!(filters.add_expression("domain:example.com").is_ok());
        let post = post("p");
        assert!(filters.hides(&post, false));
        assert!(
            filters
                .matched_rule(&post, false)
                .unwrap()
                .contains("keyword")
        );

        let mut clean = post.clone();
        clean.subreddit = "linux".into();
        clean.author = "bob".into();
        clean.title = "Unrelated".into();
        clean.body = "nothing".into();
        clean.domain = Some("news.example.com".into());
        assert!(filters.hides(&clean, false));
        clean.domain = Some("other.test".into());
        assert!(!filters.hides(&clean, false));
        assert!(!filters.hides(&clean, true));

        filters.hide_read = true;
        assert!(filters.hides(&clean, true));
        assert!(!filters.hides(&clean, false));
    }

    #[test]
    fn invalid_filter_expressions_are_rejected() {
        let mut filters = LocalFilters::default();
        assert!(filters.add_expression("").is_err());
        assert!(filters.add_expression("nonsense:x").is_err());
        assert!(filters.add_expression("sub:bad name!").is_err());
        assert!(filters.add_expression("domain:localhost").is_err());
        assert!(!filters.is_active());
    }

    #[test]
    fn malformed_library_warns_instead_of_crashing() {
        let path = temp_path("broken");
        fs::write(&path, "bookmarks = [oops").unwrap();
        let (library, warnings) = Library::load(&path);
        assert!(library.bookmarks.is_empty());
        assert_eq!(warnings.len(), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn corrupt_library_is_backed_up_and_original_survives() {
        let path = temp_path("corrupt-protected");
        fs::write(&path, "bookmarks = [oops").unwrap();
        let loaded = Library::load_checked(&path);
        assert!(loaded.corrupt);
        assert!(loaded.library.bookmarks.is_empty());
        let recovery = loaded.recovery.expect("recovery backup");
        assert!(recovery.is_file());
        assert_eq!(fs::read(&path).unwrap(), b"bookmarks = [oops");
        assert_eq!(fs::read(&recovery).unwrap(), b"bookmarks = [oops");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&recovery);
    }
}
