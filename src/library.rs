//! Local, provider-independent user library.
//!
//! Bookmarks, read state and local filters never touch Reddit. They live in a
//! `library.toml` file next to the active config so demo, Redlib, Old Reddit
//! and OAuth sessions all share the same offline-friendly data.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::{FeedSort, FeedSpec, Post};

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
        let mut warnings = Vec::new();
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), warnings);
            }
            Err(error) => {
                warnings.push(format!(
                    "Could not read {}: {error}. Bookmarks start empty.",
                    path.display()
                ));
                return (Self::default(), warnings);
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(mut library) => {
                library.normalize();
                (library, warnings)
            }
            Err(error) => {
                warnings.push(format!(
                    "Invalid library file ({}): {}. Bookmarks start empty.",
                    path.display(),
                    error.to_string().lines().next().unwrap_or("parse error")
                ));
                (Self::default(), warnings)
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create library directory: {error}"))?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|error| format!("Could not serialise library: {error}"))?;
        let tmp = path.with_extension("toml.tmp");
        {
            let mut file = fs::File::create(&tmp)
                .map_err(|error| format!("Could not write library: {error}"))?;
            file.write_all(text.as_bytes())
                .map_err(|error| format!("Could not write library: {error}"))?;
            file.flush()
                .map_err(|error| format!("Could not flush library: {error}"))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        #[cfg(windows)]
        {
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|error| format!("Could not replace library: {error}"))?;
        Ok(())
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
            self.bookmarks.truncate(500);
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
    fn last_feed_and_sort_roundtrip() {
        let path = temp_path("last-view");
        let library = Library {
            last_feed: Some(FeedSpec::Search {
                query: "rust async".into(),
                scope: SearchScope::Subreddit("rust".into()),
            }),
            last_sort: Some(FeedSort::New),
            ..Library::default()
        };
        library.save(&path).unwrap();
        let (loaded, warnings) = Library::load(&path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.last_feed, library.last_feed);
        assert_eq!(loaded.last_sort, Some(FeedSort::New));
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
}
