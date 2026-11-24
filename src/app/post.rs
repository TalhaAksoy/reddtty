use std::collections::{HashMap, HashSet};

use ratatui::text::Line;

use crate::event::ViewId;
use crate::models::{Comment, CommentSort, Post};
use crate::ui::theme::Theme;
use crate::utils::text::{sanitize, strip_markdown, wrap_text};

#[derive(Clone, Debug)]
pub struct CommentRow {
    pub id: String,
    pub depth: usize,
    pub prefix: String,
    pub body_prefix: String,
    pub author: String,
    pub score: i64,
    pub created_utc: i64,
    pub distinguished: bool,
    pub deleted: bool,
    pub removed: bool,
    pub is_more: bool,
    pub more_count: u32,
    pub more_children: Vec<String>,
    pub has_children: bool,
    pub collapsed: bool,
    pub hidden_replies: usize,
    pub body_lines: Vec<String>,
    pub height: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToggleOutcome {
    Nothing,
    Toggled,
    LoadMore { id: String, children: Vec<String> },
}

#[derive(Clone, Debug)]
pub struct PostView {
    pub id: ViewId,
    pub post: Post,
    pub post_loading: bool,
    pub post_request: Option<u64>,
    pub post_error: Option<String>,
    pub comments: Vec<Comment>,
    pub rows: Vec<CommentRow>,
    pub heights: Vec<u16>,
    pub heights_width: u16,
    pub selected: usize,
    pub offset: u32,
    pub body_offset: u32,
    pub body_cache: Option<(u16, Vec<Line<'static>>)>,
    pub comments_sort: CommentSort,
    pub comments_loading: bool,
    pub comments_request: Option<u64>,
    pub comments_error: Option<String>,
    pub collapsed: HashSet<String>,
    pub more_loading: HashSet<String>,
    pub pending_more: HashMap<String, u64>,
    pub last_comment_width: u16,
    pub last_comments_viewport: u16,
    pub last_body_viewport: u16,
    pub comments_dirty: bool,
    /// Show only comments written by the post author.
    pub op_only: bool,
}

impl PostView {
    pub fn new(id: ViewId, post: Post, comments_sort: CommentSort) -> Self {
        Self {
            id,
            post,
            post_loading: true,
            post_request: None,
            post_error: None,
            comments: Vec::new(),
            rows: Vec::new(),
            heights: Vec::new(),
            heights_width: 0,
            selected: 0,
            offset: 0,
            body_offset: 0,
            body_cache: None,
            comments_sort,
            comments_loading: true,
            comments_request: None,
            comments_error: None,
            collapsed: HashSet::new(),
            more_loading: HashSet::new(),
            pending_more: HashMap::new(),
            last_comment_width: 0,
            last_comments_viewport: 0,
            last_body_viewport: 0,
            comments_dirty: true,
            op_only: false,
        }
    }

    pub fn invalidate_body(&mut self) {
        self.body_cache = None;
    }

    pub fn ensure_body(&mut self, width: u16, theme: &Theme) {
        if width == 0 {
            return;
        }
        if self.body_cache.as_ref().is_some_and(|(w, _)| *w == width) {
            return;
        }
        let text = sanitize(&self.post.body);
        let lines = crate::utils::markdown::render(&text, width as usize, theme.md_styles());
        self.body_cache = Some((width, lines));
        self.clamp_body();
    }

    pub fn body_len(&self) -> usize {
        self.body_cache
            .as_ref()
            .map(|(_, lines)| lines.len())
            .unwrap_or(0)
    }

    pub fn body_lines(&self) -> &[Line<'static>] {
        self.body_cache
            .as_ref()
            .map(|(_, lines)| lines.as_slice())
            .unwrap_or(&[])
    }

    pub fn clamp_body(&mut self) {
        let max = self
            .body_len()
            .saturating_sub(self.last_body_viewport.max(1) as usize) as u32;
        self.body_offset = self.body_offset.min(max);
    }

    pub fn scroll_body(&mut self, delta: i32) {
        self.body_offset = (self.body_offset as i64 + delta as i64).clamp(
            0,
            self.body_len()
                .saturating_sub(self.last_body_viewport.max(1) as usize) as i64,
        ) as u32;
    }

    pub fn body_page(&mut self, down: bool) {
        let step = (self.last_body_viewport.max(1) as i32) * if down { 1 } else { -1 };
        self.scroll_body(step);
    }

    pub fn replace_comments(&mut self, comments: Vec<Comment>, width: u16) {
        self.comments = comments;
        self.comments_loading = false;
        self.comments_error = None;
        self.selected = 0;
        self.offset = 0;
        self.collapsed.clear();
        self.comments_dirty = true;
        self.rebuild_comments(width);
    }

    pub fn set_comments_error(&mut self, error: String) {
        self.comments_loading = false;
        self.comments_error = Some(error);
    }

    pub fn mark_comments_loading(&mut self) {
        self.comments_loading = true;
        self.comments_error = None;
    }

    pub fn rebuild_comments(&mut self, width: u16) {
        if width == 0 {
            return;
        }
        if !self.comments_dirty && self.last_comment_width == width {
            return;
        }
        let keep_id = self.rows.get(self.selected).map(|r| r.id.clone());
        let op_author = self.op_only.then(|| self.post.author.clone());
        let mut rows = Vec::new();
        for root in &self.comments {
            push_rows(
                root,
                0,
                width,
                &self.collapsed,
                &self.more_loading,
                self.comments_loading,
                op_author.as_deref(),
                &mut rows,
            );
        }
        self.heights = rows.iter().map(|r| r.height).collect();
        self.rows = rows;
        self.last_comment_width = width;
        self.comments_dirty = false;
        self.selected = keep_id
            .and_then(|id| self.rows.iter().position(|r| r.id == id))
            .unwrap_or_else(|| self.selected.min(self.rows.len().saturating_sub(1)));
        self.clamp_comment_offset();
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

    pub fn clamp_comment_offset(&mut self) {
        let viewport = self.last_comments_viewport.max(1);
        let max = self.total_height().saturating_sub(viewport as u32);
        self.offset = self.offset.min(max);
        if self.rows.is_empty() {
            self.selected = 0;
            self.offset = 0;
        } else {
            self.selected = self.selected.min(self.rows.len() - 1);
        }
    }

    pub fn ensure_comment_visible(&mut self, viewport: u16) {
        self.last_comments_viewport = viewport;
        if self.rows.is_empty() || viewport == 0 {
            return;
        }
        self.selected = self.selected.min(self.rows.len() - 1);
        let start = self.prefix_height(self.selected);
        let end = start + self.heights.get(self.selected).copied().unwrap_or(1) as u32;
        let viewport = viewport as u32;
        if start < self.offset {
            self.offset = start;
        } else if end > self.offset + viewport {
            self.offset = end.saturating_sub(viewport);
        }
        let max = self.total_height().saturating_sub(viewport);
        self.offset = self.offset.min(max);
    }

    pub fn move_comment_selection(&mut self, delta: isize, viewport: u16) {
        if self.rows.is_empty() {
            return;
        }
        let max = self.rows.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
        self.ensure_comment_visible(viewport);
    }

    pub fn comment_page(&mut self, down: bool, viewport: u16) {
        if self.rows.is_empty() {
            return;
        }
        let step = (viewport / 2).max(1) as isize;
        self.move_comment_selection(if down { step } else { -step }, viewport);
    }

    pub fn comment_home(&mut self, viewport: u16) {
        self.selected = 0;
        self.offset = 0;
        self.ensure_comment_visible(viewport);
    }

    pub fn comment_end(&mut self, viewport: u16) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = self.rows.len() - 1;
        self.ensure_comment_visible(viewport);
    }

    pub fn scroll_comments(&mut self, delta: i32, viewport: u16) {
        self.last_comments_viewport = viewport;
        if self.rows.is_empty() || viewport == 0 {
            return;
        }
        let max = self.total_height().saturating_sub(viewport as u32);
        self.offset = (self.offset as i64 + delta as i64).clamp(0, max as i64) as u32;
        let selected_start = self.prefix_height(self.selected);
        let selected_end =
            selected_start + self.heights.get(self.selected).copied().unwrap_or(1) as u32;
        if (selected_end <= self.offset || selected_start >= self.offset + viewport as u32)
            && let Some(index) = self.comment_at_line(self.offset)
        {
            self.selected = index;
            self.ensure_comment_visible(viewport);
        }
    }

    pub fn comment_at_line(&self, line: u32) -> Option<usize> {
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

    pub fn visible_comments(&self, viewport: u16) -> std::ops::Range<usize> {
        if self.rows.is_empty() {
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
        let start = start.unwrap_or_else(|| self.rows.len().saturating_sub(1));
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

    pub fn toggle_selected(&mut self, width: u16) -> ToggleOutcome {
        let Some(row) = self.rows.get(self.selected).cloned() else {
            return ToggleOutcome::Nothing;
        };
        if row.is_more {
            if row.more_children.is_empty() {
                return ToggleOutcome::Nothing;
            }
            if self.more_loading.contains(&row.id) {
                return ToggleOutcome::Nothing;
            }
            return ToggleOutcome::LoadMore {
                id: row.id,
                children: row.more_children,
            };
        }
        if !row.has_children {
            return ToggleOutcome::Nothing;
        }
        if self.collapsed.contains(&row.id) {
            self.collapsed.remove(&row.id);
        } else {
            self.collapsed.insert(row.id);
        }
        self.comments_dirty = true;
        self.rebuild_comments(width);
        ToggleOutcome::Toggled
    }

    pub fn mark_more_loading(&mut self, id: &str, loading: bool, width: u16) {
        if loading {
            self.more_loading.insert(id.to_string());
        } else {
            self.more_loading.remove(id);
        }
        self.comments_dirty = true;
        self.rebuild_comments(width);
    }

    pub fn apply_more(&mut self, more_id: &str, loaded: Vec<Comment>, width: u16) {
        self.more_loading.remove(more_id);
        self.pending_more.remove(more_id);
        replace_more_in_roots(&mut self.comments, more_id, loaded);
        self.comments_dirty = true;
        self.rebuild_comments(width);
    }

    pub fn comment_row_index(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    pub fn comment_scrollbar(&self, viewport: u16) -> (u32, u32, u16) {
        (self.offset, self.total_height(), viewport)
    }

    pub fn click_comment(&mut self, index: usize, viewport: u16) {
        if index < self.rows.len() {
            self.selected = index;
            self.ensure_comment_visible(viewport);
        }
    }

    pub fn selected_comment_id(&self) -> Option<&str> {
        self.rows
            .get(self.selected)
            .filter(|row| !row.is_more)
            .map(|row| row.id.as_str())
    }

    /// Move to the next top-level comment. Returns false at the last root.
    pub fn next_root(&mut self, viewport: u16) -> bool {
        if let Some(index) = self
            .rows
            .iter()
            .enumerate()
            .skip(self.selected + 1)
            .find_map(|(index, row)| (row.depth == 0).then_some(index))
        {
            self.selected = index;
            self.ensure_comment_visible(viewport);
            true
        } else {
            false
        }
    }

    /// Move to the previous top-level comment. Returns false at the first root.
    pub fn prev_root(&mut self, viewport: u16) -> bool {
        if let Some(index) = self.rows[..self.selected]
            .iter()
            .rposition(|row| row.depth == 0)
        {
            self.selected = index;
            self.ensure_comment_visible(viewport);
            true
        } else {
            false
        }
    }

    /// Move to the nearest visible parent comment. Returns false on a root.
    pub fn jump_to_parent(&mut self, viewport: u16) -> bool {
        let Some(current) = self.rows.get(self.selected) else {
            return false;
        };
        if current.depth == 0 {
            return false;
        }
        let depth = current.depth;
        if let Some(index) = self.rows[..self.selected]
            .iter()
            .rposition(|row| row.depth < depth)
        {
            self.selected = index;
            self.ensure_comment_visible(viewport);
            true
        } else {
            false
        }
    }

    /// Collapse every root that has replies, or expand all when they are all
    /// collapsed. Returns true when the tree is now collapsed.
    pub fn toggle_collapse_all(&mut self, width: u16) -> bool {
        let roots: Vec<String> = self
            .comments
            .iter()
            .filter(|comment| !comment.replies.is_empty())
            .map(|comment| comment.id.clone())
            .collect();
        if roots.is_empty() {
            return false;
        }
        let all_collapsed = roots.iter().all(|id| self.collapsed.contains(id));
        if all_collapsed {
            self.collapsed.clear();
        } else {
            self.collapsed.extend(roots);
        }
        self.comments_dirty = true;
        self.rebuild_comments(width);
        !all_collapsed
    }

    /// Toggle the OP-only filter. Returns true when it is enabled.
    pub fn toggle_op_only(&mut self, width: u16) -> bool {
        self.op_only = !self.op_only;
        self.comments_dirty = true;
        self.rebuild_comments(width);
        self.op_only
    }

    /// Jump to the next comment that is not in the local read set. Returns the
    /// comment id that should be marked read by the caller.
    pub fn next_unread_comment(
        &mut self,
        read: &std::collections::BTreeSet<String>,
        viewport: u16,
    ) -> Option<String> {
        let index = self
            .rows
            .iter()
            .enumerate()
            .skip(self.selected + 1)
            .find_map(|(index, row)| (!row.is_more && !read.contains(&row.id)).then_some(index))?;
        self.selected = index;
        self.ensure_comment_visible(viewport);
        self.rows.get(index).map(|row| row.id.clone())
    }

    #[cfg(test)]
    pub fn with_comments(
        id: ViewId,
        post: Post,
        comments: Vec<Comment>,
        sort: CommentSort,
    ) -> Self {
        let mut view = Self::new(id, post, sort);
        view.comments = comments;
        view.comments_loading = false;
        view.comments_dirty = true;
        view
    }
}

#[allow(clippy::too_many_arguments)]
fn push_rows(
    comment: &Comment,
    depth: usize,
    width: u16,
    collapsed_set: &HashSet<String>,
    more_loading: &HashSet<String>,
    comments_loading: bool,
    op_author: Option<&str>,
    out: &mut Vec<CommentRow>,
) {
    let display_depth = depth.min(8);
    let (prefix, body_prefix) = if display_depth == 0 {
        (String::new(), String::new())
    } else {
        let rails = "│ ".repeat(display_depth.saturating_sub(1));
        let prefix = format!("{rails}├─ ");
        let body_prefix = format!("{}   ", "│ ".repeat(display_depth));
        (prefix, body_prefix)
    };
    let prefix_width = crate::utils::text::width(&body_prefix) as u16;
    let body_width = width.saturating_sub(prefix_width + 1).max(8);

    if comment.is_more {
        if op_author.is_some() {
            return;
        }
        let loading = more_loading.contains(&comment.id);
        let label = if loading {
            "loading more replies…".to_string()
        } else if comment.more_count > 0 {
            format!("load {} more replies", comment.more_count)
        } else {
            "load more replies".to_string()
        };
        let body_lines = if comments_loading {
            vec!["waiting for comments…".to_string()]
        } else {
            Vec::new()
        };
        out.push(CommentRow {
            id: comment.id.clone(),
            depth: display_depth,
            prefix,
            body_prefix,
            author: label,
            score: 0,
            created_utc: 0,
            distinguished: false,
            deleted: false,
            removed: false,
            is_more: true,
            more_count: comment.more_count,
            more_children: comment.more_children.clone(),
            has_children: false,
            collapsed: false,
            hidden_replies: comment.more_count as usize,
            height: (1 + body_lines.len()) as u16,
            body_lines,
        });
        return;
    }

    let collapsed = collapsed_set.contains(&comment.id);
    let body_source = if comment.deleted {
        "[deleted]".to_string()
    } else if comment.removed {
        "[removed]".to_string()
    } else {
        strip_markdown(&sanitize(&comment.body))
    };
    let body_lines = if collapsed {
        Vec::new()
    } else {
        let mut lines = wrap_text(&body_source, body_width as usize);
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines.truncate(120);
        lines
    };
    let hidden_replies = if collapsed {
        comment.descendant_count()
    } else {
        0
    };
    let height = 1 + body_lines.len() as u16;
    let is_op = op_author
        .map(|author| comment.author.eq_ignore_ascii_case(author))
        .unwrap_or(true);
    if is_op {
        let row = CommentRow {
            id: comment.id.clone(),
            depth: display_depth,
            prefix,
            body_prefix,
            author: sanitize(&comment.author),
            score: comment.score,
            created_utc: comment.created_utc,
            distinguished: comment
                .distinguished
                .as_deref()
                .is_some_and(|d| !d.is_empty() && d != "null"),
            deleted: comment.deleted,
            removed: comment.removed,
            is_more: false,
            more_count: 0,
            more_children: Vec::new(),
            has_children: !comment.replies.is_empty(),
            collapsed,
            hidden_replies,
            body_lines,
            height,
        };
        out.push(row);
    }

    // In OP-only mode, non-OP parents are skipped but their OP descendants must
    // still be reachable, so recurse even when the hidden parent is collapsed.
    if !collapsed || (op_author.is_some() && !is_op) {
        for reply in &comment.replies {
            push_rows(
                reply,
                depth + 1,
                width,
                collapsed_set,
                more_loading,
                comments_loading,
                op_author,
                out,
            );
        }
    }
}

fn replace_more_in_roots(roots: &mut Vec<Comment>, more_id: &str, loaded: Vec<Comment>) -> bool {
    if let Some(index) = roots.iter().position(|r| r.id == more_id) {
        roots.splice(index..=index, loaded);
        return true;
    }
    for root in roots {
        if root.replace_more(more_id, loaded.clone()) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PostContent;

    fn post() -> Post {
        Post {
            id: "p".into(),
            subreddit: "rust".into(),
            author: "a".into(),
            title: "Title".into(),
            body: "# Body\n\nHello **world**".into(),
            created_utc: 0,
            score: 1,
            num_comments: 3,
            permalink: "/r/rust/comments/p".into(),
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

    fn tree() -> Vec<Comment> {
        let mut root = Comment::new("a", "alice", "Root comment");
        let mut child = Comment::new("b", "bob", "Child comment");
        child.replies.push(Comment::new("c", "carol", "Grandchild"));
        root.replies.push(child);
        let mut second = Comment::new("d", "dave", "Second root");
        second
            .replies
            .push(Comment::more("m", 4, vec!["x".into(), "y".into()]));
        vec![root, second]
    }

    #[test]
    fn rows_are_nested_and_indented() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        let ids: Vec<_> = view.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c", "d", "m"]);
        assert_eq!(view.rows[1].depth, 1);
        assert_eq!(view.rows[2].depth, 2);
        assert!(view.rows[1].prefix.contains("├─"));
    }

    #[test]
    fn collapse_hides_descendants_and_reports_count() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        view.selected = 1;
        let outcome = view.toggle_selected(70);
        assert_eq!(outcome, ToggleOutcome::Toggled);
        assert_eq!(view.rows.len(), 4);
        assert!(view.rows[1].collapsed);
        assert_eq!(view.rows[1].hidden_replies, 1);
        view.toggle_selected(70);
        assert_eq!(view.rows.len(), 5);
    }

    #[test]
    fn more_node_requests_children() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        view.selected = view.comment_row_index("m").unwrap();
        assert_eq!(
            view.toggle_selected(70),
            ToggleOutcome::LoadMore {
                id: "m".into(),
                children: vec!["x".into(), "y".into()]
            }
        );
    }

    #[test]
    fn applying_more_splices_replies() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        view.mark_more_loading("m", true, 70);
        assert!(view.rows.iter().any(|r| r.is_more));
        view.apply_more("m", vec![Comment::new("x", "xavier", "Loaded reply")], 70);
        assert!(view.rows.iter().all(|r| r.id != "m"));
        assert!(view.rows.iter().any(|r| r.id == "x"));
    }

    #[test]
    fn root_and_parent_navigation_follow_the_tree() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        assert_eq!(view.rows[view.selected].id, "a");
        assert!(view.next_root(10));
        assert_eq!(view.rows[view.selected].id, "d");
        assert!(!view.next_root(10), "last root has no successor");
        assert!(view.prev_root(10));
        assert_eq!(view.rows[view.selected].id, "a");
        assert!(!view.prev_root(10));

        view.selected = 2;
        assert_eq!(view.rows[view.selected].id, "c");
        assert!(view.jump_to_parent(10));
        assert_eq!(view.rows[view.selected].id, "b");
        assert!(view.jump_to_parent(10));
        assert_eq!(view.rows[view.selected].id, "a");
        assert!(!view.jump_to_parent(10));
    }

    #[test]
    fn collapse_all_toggles_every_root() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        assert!(view.toggle_collapse_all(70));
        assert_eq!(
            view.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "d"]
        );
        assert!(!view.toggle_collapse_all(70));
        assert_eq!(view.rows.len(), 5);
    }

    #[test]
    fn op_only_keeps_the_author_and_their_visible_descendants() {
        let mut post = post();
        post.author = "alice".into();
        let mut view = PostView::with_comments(ViewId(1), post, tree(), CommentSort::Best);
        view.rebuild_comments(70);
        assert!(view.toggle_op_only(70));
        assert_eq!(
            view.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["a"],
            "only alice's comment should remain"
        );
        assert!(!view.toggle_op_only(70));
        assert!(view.rows.len() > 2);
    }

    #[test]
    fn next_unread_comment_skips_read_rows() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(70);
        let mut read = std::collections::BTreeSet::new();
        read.insert("b".to_string());
        assert_eq!(view.next_unread_comment(&read, 10).as_deref(), Some("c"));
        assert_eq!(view.rows[view.selected].id, "c");
        read.insert("c".to_string());
        assert_eq!(view.next_unread_comment(&read, 10).as_deref(), Some("d"));
        read.insert("d".to_string());
        assert!(view.next_unread_comment(&read, 10).is_none());
    }

    #[test]
    fn body_is_cached_and_scrolls() {
        let mut view = PostView::new(ViewId(1), post(), CommentSort::Best);
        view.last_body_viewport = 2;
        view.ensure_body(40, &Theme::dark());
        assert!(view.body_len() > 2);
        view.scroll_body(1000);
        assert!(view.body_offset > 0);
        view.scroll_body(-1000);
        assert_eq!(view.body_offset, 0);
    }

    #[test]
    fn comment_selection_scrolls() {
        let mut view = PostView::with_comments(ViewId(1), post(), tree(), CommentSort::Best);
        view.rebuild_comments(60);
        for _ in 0..3 {
            view.move_comment_selection(1, 4);
        }
        assert_eq!(view.selected, 3);
        assert!(view.offset > 0);
    }
}
