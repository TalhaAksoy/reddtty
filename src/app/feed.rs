use std::collections::HashSet;
use std::time::Instant;

use crate::config::Settings;
use crate::event::ViewId;
use crate::models::{FeedPage, FeedSort, FeedSpec, Post, Subreddit, TopRange};
use crate::ui::widgets::post_card;

/// State for one feed screen. The view stays alive on the navigation stack so
/// scroll position and loaded pages survive a trip into a post.
#[derive(Clone, Debug)]
pub struct FeedView {
    pub id: ViewId,
    pub spec: FeedSpec,
    pub sort: FeedSort,
    pub top_range: TopRange,
    pub posts: Vec<Post>,
    pub selected: usize,
    pub offset: u32,
    pub heights: Vec<u16>,
    pub heights_width: u16,
    pub heights_compact: bool,
    pub heights_dirty: bool,
    pub after: Option<String>,
    pub has_more: bool,
    /// Set when an append request failed. Automatic pagination stays paused
    /// so a broken source cannot trigger a request storm; the user can retry
    /// deliberately without losing the existing feed or cursor.
    pub more_error: Option<String>,
    /// Cooldown from a provider's Retry-After header.
    pub retry_after_until: Option<Instant>,
    /// Consecutive append pages that contained no new post IDs.
    pub duplicate_pages: u32,
    pub loading_initial: bool,
    pub loading_more: bool,
    pub refreshing: bool,
    pub error: Option<String>,
    pub request: Option<u64>,
    pub more_request: Option<u64>,
    pub subreddit: Option<Subreddit>,
    pub subreddit_request: Option<u64>,
    pub subreddit_error: Option<String>,
    pub pending_selected_id: Option<String>,
    pub last_viewport: u16,
}

impl FeedView {
    pub fn new(id: ViewId, spec: FeedSpec, sort: FeedSort, top_range: TopRange) -> Self {
        Self {
            id,
            spec,
            sort,
            top_range,
            posts: Vec::new(),
            selected: 0,
            offset: 0,
            heights: Vec::new(),
            heights_width: 0,
            heights_compact: false,
            heights_dirty: true,
            after: None,
            has_more: false,
            more_error: None,
            retry_after_until: None,
            duplicate_pages: 0,
            loading_initial: true,
            loading_more: false,
            refreshing: false,
            error: None,
            request: None,
            more_request: None,
            subreddit: None,
            subreddit_request: None,
            subreddit_error: None,
            pending_selected_id: None,
            last_viewport: 0,
        }
    }

    pub fn selected_post(&self) -> Option<&Post> {
        self.posts.get(self.selected)
    }

    pub fn selected_id(&self) -> Option<String> {
        self.selected_post().map(|p| p.id.clone())
    }

    pub fn invalidate_heights(&mut self) {
        self.heights_dirty = true;
    }

    pub fn reset_for_query(&mut self, sort: FeedSort, top_range: TopRange) {
        self.sort = sort;
        self.top_range = top_range;
        self.posts.clear();
        self.heights.clear();
        self.selected = 0;
        self.offset = 0;
        self.after = None;
        self.has_more = false;
        self.more_error = None;
        self.retry_after_until = None;
        self.duplicate_pages = 0;
        self.loading_initial = true;
        self.loading_more = false;
        self.refreshing = false;
        self.error = None;
        self.request = None;
        self.more_request = None;
        self.pending_selected_id = None;
        self.heights_dirty = true;
        self.subreddit = None;
        self.subreddit_error = None;
        self.subreddit_request = None;
    }

    pub fn recompute_heights(
        &mut self,
        card_width: u16,
        compact: bool,
        settings: &Settings,
        preview_enabled: bool,
        revealed: &HashSet<String>,
    ) {
        if !self.heights_dirty
            && self.heights_width == card_width
            && self.heights_compact == compact
            && self.heights.len() == self.posts.len()
        {
            return;
        }
        self.heights = self
            .posts
            .iter()
            .map(|post| {
                let show = revealed.contains(&post.id);
                let hidden = post.is_hidden(
                    settings.content.show_nsfw || show,
                    !settings.content.hide_spoilers || show,
                );
                post_card::card_height(post, card_width, compact, preview_enabled, hidden)
            })
            .collect();
        self.heights_width = card_width;
        self.heights_compact = compact;
        self.heights_dirty = false;
        self.clamp_offset();
    }

    pub fn total_height(&self) -> u32 {
        self.heights.iter().map(|h| *h as u32 + 1).sum()
    }

    pub fn prefix_height(&self, index: usize) -> u32 {
        self.heights
            .iter()
            .take(index.min(self.heights.len()))
            .map(|h| *h as u32 + 1)
            .sum()
    }

    pub fn max_offset(&self, viewport: u16) -> u32 {
        self.total_height().saturating_sub(viewport.max(1) as u32)
    }

    pub fn clamp_offset(&mut self) {
        if self.posts.is_empty() {
            self.selected = 0;
            self.offset = 0;
        } else {
            self.selected = self.selected.min(self.posts.len() - 1);
            self.offset = self.offset.min(self.max_offset(self.last_viewport));
        }
    }

    pub fn ensure_selected_visible(&mut self, viewport: u16) {
        self.last_viewport = viewport;
        if self.posts.is_empty() || viewport == 0 {
            return;
        }
        self.selected = self.selected.min(self.posts.len() - 1);
        let start = self.prefix_height(self.selected);
        let end = start + self.heights.get(self.selected).copied().unwrap_or(1) as u32;
        let viewport = viewport as u32;
        // Only snap when the selected card is completely outside the
        // viewport. Line scrolling inside a tall card must survive prepare().
        if end <= self.offset || start >= self.offset.saturating_add(viewport) {
            self.offset = start;
        }
        self.offset = self.offset.min(self.max_offset(viewport as u16));
    }

    pub fn move_selection(&mut self, delta: isize, viewport: u16) {
        if self.posts.is_empty() {
            return;
        }
        let max = self.posts.len() - 1;
        let new = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
        if new != self.selected {
            self.selected = new;
            self.ensure_selected_visible(viewport);
        }
    }

    pub fn page_move(&mut self, down: bool, viewport: u16) {
        if self.posts.is_empty() {
            return;
        }
        let step = (viewport / 2).max(1) as i32;
        self.scroll_lines(if down { step } else { -step }, viewport);
    }

    pub fn go_home(&mut self, viewport: u16) {
        if self.posts.is_empty() {
            return;
        }
        self.selected = 0;
        self.offset = 0;
        self.ensure_selected_visible(viewport);
    }

    pub fn go_end(&mut self, viewport: u16) {
        if self.posts.is_empty() {
            return;
        }
        self.selected = self.posts.len() - 1;
        self.last_viewport = viewport;
        self.offset = self.max_offset(viewport);
    }

    /// Wheel scrolling moves a card at a time. Cards have variable heights
    /// and the renderer avoids partial cards so the header is never painted
    /// over; keyboard navigation offers line/page granularity within a card.
    pub fn scroll_lines(&mut self, delta: i32, viewport: u16) {
        self.last_viewport = viewport;
        if self.posts.is_empty() || viewport == 0 || delta == 0 {
            return;
        }
        let max = self.max_offset(viewport) as i64;
        let next = (self.offset as i64 + delta as i64).clamp(0, max) as u32;
        self.offset = next;
        let selected_start = self.prefix_height(self.selected);
        let selected_end =
            selected_start + self.heights.get(self.selected).copied().unwrap_or(1) as u32;
        if (selected_end <= next || selected_start >= next.saturating_add(viewport as u32))
            && let Some(index) = self.index_at_line(next)
        {
            self.selected = index;
        }
    }

    pub fn index_at_line(&self, line: u32) -> Option<usize> {
        let mut y = 0u32;
        for (index, height) in self.heights.iter().enumerate() {
            let end = y + *height as u32;
            if line >= y && line < end {
                return Some(index);
            }
            y = end + 1;
        }
        None
    }

    pub fn visible_range(&self, viewport: u16) -> std::ops::Range<usize> {
        if self.posts.is_empty() {
            return 0..0;
        }
        let top = self.offset;
        let bottom = self.offset + viewport as u32;
        let mut start = None;
        let mut y = 0u32;
        for (index, height) in self.heights.iter().enumerate() {
            let end = y + *height as u32;
            if end > top {
                start = Some(index);
                break;
            }
            y = end + 1;
        }
        let start = start.unwrap_or_else(|| self.posts.len().saturating_sub(1));
        let mut end = start;
        let mut y = self.prefix_height(start);
        for (index, height) in self.heights.iter().enumerate().skip(start) {
            y = y.saturating_add(*height as u32 + 1);
            end = index + 1;
            if y >= bottom {
                break;
            }
        }
        start..end.max(start)
    }

    pub fn scrollbar(&self, viewport: u16) -> (u32, u32, u16) {
        (self.offset, self.total_height(), viewport)
    }

    pub fn wants_more(&self, viewport: u16) -> bool {
        if self.more_error.is_some()
            || self.refreshing
            || self.request.is_some()
            || self.error.is_some()
            || self.retry_wait_secs().is_some()
        {
            return false;
        }
        if !self.has_more
            || self.loading_more
            || self.more_request.is_some()
            || self.after.is_none()
            || self.heights.len() != self.posts.len()
        {
            return false;
        }
        // A filter can hide every post on a page. Keep paginating so the feed
        // can fill instead of getting stuck on a legitimately empty result.
        if self.posts.is_empty() {
            return true;
        }
        if self.total_height() == 0 {
            return false;
        }
        if self.selected + 3 >= self.posts.len() {
            return true;
        }
        self.offset + viewport as u32 + 4 >= self.total_height()
    }

    pub fn apply_page(&mut self, page: FeedPage, append: bool, reset_position: bool) {
        self.loading_initial = false;
        self.loading_more = false;
        self.refreshing = false;
        self.error = None;
        self.more_error = None;
        self.retry_after_until = None;

        if append {
            let mut seen: HashSet<String> = self.posts.iter().map(|p| p.id.clone()).collect();
            let old_after = self.after.clone();
            let mut added = 0usize;
            for post in page.posts {
                if seen.insert(post.id.clone()) {
                    self.posts.push(post);
                    added += 1;
                }
            }
            let same_cursor = old_after.is_some() && old_after == page.after;
            if added == 0 {
                self.duplicate_pages = self.duplicate_pages.saturating_add(1);
            } else {
                self.duplicate_pages = 0;
            }
            // A duplicate-only page with a fresh cursor is progress for a
            // short while, but a two-cursor cycle or a permanently repeated
            // listing must not create an infinite request loop.
            self.has_more = page.has_more && !same_cursor && self.duplicate_pages < 3;
            self.after = if self.has_more { page.after } else { None };
            if self.duplicate_pages >= 3 {
                self.more_error =
                    Some("the source returned only duplicate posts; refresh the feed".into());
            }
            self.heights_dirty = true;
            return;
        }
        self.duplicate_pages = 0;

        let pending = self.pending_selected_id.take();
        let keep_position = pending.is_some();
        let mut seen = HashSet::new();
        self.posts = page
            .posts
            .into_iter()
            .filter(|post| seen.insert(post.id.clone()))
            .collect();
        self.after = page.after;
        self.has_more = page.has_more;
        self.heights_dirty = true;

        let target = pending
            .and_then(|id| self.posts.iter().position(|p| p.id == id))
            .unwrap_or(0);
        if reset_position || (!keep_position && target == 0) {
            self.selected = target;
            self.offset = 0;
        } else {
            self.selected = target;
            self.clamp_offset();
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_initial = false;
        self.loading_more = false;
        self.refreshing = false;
        self.more_error = None;
        self.retry_after_until = None;
        self.error = Some(error);
    }

    pub fn set_more_error(&mut self, error: String) {
        self.loading_more = false;
        self.more_error = Some(error);
    }

    pub fn set_retry_after(&mut self, seconds: Option<u64>) {
        self.retry_after_until = seconds
            .map(|seconds| Instant::now() + std::time::Duration::from_secs(seconds.min(86_400)));
    }

    pub fn retry_wait_secs(&self) -> Option<u64> {
        self.retry_after_until.and_then(|until| {
            let now = Instant::now();
            (until > now).then(|| (until - now).as_secs().max(1))
        })
    }

    #[cfg(test)]
    pub fn with_posts(id: ViewId, posts: Vec<Post>) -> Self {
        let mut view = Self::new(id, FeedSpec::Home, FeedSort::Hot, TopRange::Week);
        view.posts = posts;
        view.loading_initial = false;
        view.has_more = false;
        view.heights_dirty = true;
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PostContent;

    fn post(id: &str) -> Post {
        Post {
            id: id.into(),
            subreddit: "rust".into(),
            author: "alice".into(),
            title: "A title".into(),
            body: "body ".repeat(40),
            created_utc: 0,
            score: 10,
            num_comments: 2,
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

    fn view() -> FeedView {
        let posts = (0..30).map(|i| post(&format!("p{i}"))).collect();
        FeedView::with_posts(ViewId(1), posts)
    }

    #[test]
    fn heights_are_recomputed_and_offsets_clamped() {
        let mut v = view();
        let settings = Settings::default();
        v.recompute_heights(60, false, &settings, true, &HashSet::new());
        assert_eq!(v.heights.len(), 30);
        assert!(v.total_height() > 0);
        v.offset = 999_999;
        v.clamp_offset();
        assert!(v.offset <= v.total_height());
    }

    #[test]
    fn selection_scrolls_to_stay_visible() {
        let mut v = view();
        let settings = Settings::default();
        v.recompute_heights(60, false, &settings, true, &HashSet::new());
        v.ensure_selected_visible(10);
        for _ in 0..10 {
            v.move_selection(1, 10);
        }
        assert_eq!(v.selected, 10);
        assert!(v.offset > 0);
        let start = v.prefix_height(v.selected);
        assert!(start >= v.offset);
        assert!(start < v.offset + 10);
    }

    #[test]
    fn page_and_home_end_navigation() {
        let mut v = view();
        v.ensure_selected_visible(8);
        v.go_end(8);
        assert_eq!(v.selected, 29);
        v.go_home(8);
        assert_eq!(v.selected, 0);
        assert_eq!(v.offset, 0);
        v.heights = vec![3; v.posts.len()];
        v.heights_width = 80;
        v.heights_dirty = false;
        v.page_move(true, 8);
        assert!(v.offset > 0);
    }

    #[test]
    fn append_deduplicates_and_detects_same_cursor() {
        let mut v = view();
        v.has_more = true;
        v.after = Some("a".into());
        let page = FeedPage {
            posts: vec![post("p0"), post("new")],
            after: Some("a".into()),
            has_more: true,
        };
        v.apply_page(page, true, false);
        assert_eq!(v.posts.len(), 31);
        assert!(v.posts.iter().any(|p| p.id == "new"));
        assert!(!v.has_more, "same cursor must stop pagination");
    }

    #[test]
    fn replace_preserves_selected_id_on_refresh() {
        let mut v = view();
        let settings = Settings::default();
        v.recompute_heights(60, false, &settings, true, &HashSet::new());
        v.selected = 12;
        v.pending_selected_id = Some("p12".into());
        let page = FeedPage {
            posts: (0..30).map(|i| post(&format!("p{i}"))).collect(),
            after: None,
            has_more: false,
        };
        v.apply_page(page, false, false);
        assert_eq!(v.selected, 12);
    }

    #[test]
    fn pagination_waits_for_heights_and_selection() {
        let mut v = view();
        v.has_more = true;
        v.after = Some("d:25".into());
        assert!(!v.wants_more(10), "no heights means no pagination yet");
        let settings = Settings::default();
        v.recompute_heights(60, false, &settings, true, &HashSet::new());
        v.ensure_selected_visible(10);
        assert!(
            !v.wants_more(10),
            "middle of a large feed should not load more"
        );
        v.go_end(10);
        assert!(v.wants_more(10), "last cards should request the next page");
    }

    #[test]
    fn visible_range_is_bounded() {
        let mut v = view();
        let settings = Settings::default();
        v.recompute_heights(60, false, &settings, true, &HashSet::new());
        let range = v.visible_range(12);
        assert!(range.start < v.posts.len());
        assert!(range.end <= v.posts.len());
        assert!(range.end > range.start);
    }
}

#[cfg(test)]
mod line_scroll_tests {
    use super::*;

    fn post(id: &str) -> Post {
        Post {
            id: id.into(),
            subreddit: "rust".into(),
            author: "a".into(),
            title: "t".into(),
            body: String::new(),
            created_utc: 0,
            score: 0,
            num_comments: 0,
            permalink: "/r/rust/x".into(),
            content: crate::models::PostContent::Text,
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
    fn a_tall_card_can_be_scrolled_to_its_last_line() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("tall"), post("next")]);
        view.heights = vec![40, 12];
        view.heights_width = 80;
        view.heights_dirty = false;
        view.last_viewport = 10;
        view.scroll_lines(5, 10);
        assert_eq!(view.offset, 5);
        assert_eq!(view.selected, 0);
        view.scroll_lines(1000, 10);
        assert_eq!(view.offset, view.max_offset(10));
        assert!(view.offset > 40, "the bottom of the tall card is reachable");
        view.scroll_lines(-1000, 10);
        assert_eq!(view.offset, 0);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn scrolling_into_another_card_moves_selection_without_losing_the_offset() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("a"), post("b")]);
        view.heights = vec![30, 30];
        view.heights_width = 80;
        view.heights_dirty = false;
        view.last_viewport = 10;
        view.scroll_lines(31, 10);
        assert_eq!(view.selected, 1);
        assert_eq!(view.offset, 31);
    }
}

#[cfg(test)]
mod duplicate_page_tests {
    use super::*;

    fn post(id: &str) -> Post {
        Post {
            id: id.into(),
            subreddit: "rust".into(),
            author: "a".into(),
            title: "t".into(),
            body: String::new(),
            created_utc: 0,
            score: 0,
            num_comments: 0,
            permalink: "/r/rust/x".into(),
            content: crate::models::PostContent::Text,
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
    fn duplicate_only_page_with_a_fresh_cursor_keeps_paginating() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("a"), post("b")]);
        view.after = Some("cursor-1".into());
        view.has_more = true;
        view.apply_page(
            FeedPage {
                posts: vec![post("b")],
                after: Some("cursor-2".into()),
                has_more: true,
            },
            true,
            false,
        );
        assert!(view.has_more);
        assert_eq!(view.after.as_deref(), Some("cursor-2"));
    }

    #[test]
    fn repeated_cursor_stops_pagination() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("a")]);
        view.after = Some("cursor-1".into());
        view.has_more = true;
        view.apply_page(
            FeedPage {
                posts: vec![post("a")],
                after: Some("cursor-1".into()),
                has_more: true,
            },
            true,
            false,
        );
        assert!(!view.has_more);
        assert!(view.after.is_none());
    }
}

#[cfg(test)]
mod pagination_cycle_tests {
    use super::*;

    fn post(id: &str) -> Post {
        Post {
            id: id.into(),
            subreddit: "rust".into(),
            author: "a".into(),
            title: "t".into(),
            body: String::new(),
            created_utc: 0,
            score: 0,
            num_comments: 0,
            permalink: "/r/rust/x".into(),
            content: crate::models::PostContent::Text,
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
    fn a_two_cursor_duplicate_cycle_stops_after_three_pages() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("a"), post("b")]);
        view.after = Some("cursor-1".into());
        view.has_more = true;
        for (index, cursor) in ["cursor-2", "cursor-1", "cursor-2"].iter().enumerate() {
            view.apply_page(
                FeedPage {
                    posts: vec![post("a"), post("b")],
                    after: Some((*cursor).into()),
                    has_more: true,
                },
                true,
                false,
            );
            if index < 2 {
                assert!(view.has_more, "page {index}");
            }
        }
        assert!(!view.has_more);
        assert!(view.after.is_none());
        assert!(view.more_error.is_some());
    }

    #[test]
    fn refresh_or_rate_limit_blocks_automatic_append() {
        let mut view = FeedView::with_posts(ViewId(1), vec![post("a")]);
        view.after = Some("cursor".into());
        view.has_more = true;
        view.refreshing = true;
        assert!(!view.wants_more(10));
        view.refreshing = false;
        view.set_retry_after(Some(30));
        assert!(!view.wants_more(10));
        view.retry_after_until = None;
        view.error = Some("refresh failed".into());
        assert!(!view.wants_more(10));
    }
}
