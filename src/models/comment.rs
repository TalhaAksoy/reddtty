use serde::{Deserialize, Serialize};

/// Comment ordering. These are the sort names Reddit exposes for comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSort {
    Best,
    Top,
    New,
    Controversial,
    Old,
}

impl CommentSort {
    pub const ALL: [Self; 5] = [
        Self::Best,
        Self::Top,
        Self::New,
        Self::Controversial,
        Self::Old,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Best => "Best",
            Self::Top => "Top",
            Self::New => "New",
            Self::Controversial => "Controversial",
            Self::Old => "Old",
        }
    }

    /// Reddit's comment sort value.
    pub fn api_value(self) -> &'static str {
        match self {
            Self::Best => "confidence",
            Self::Top => "top",
            Self::New => "new",
            Self::Controversial => "controversial",
            Self::Old => "old",
        }
    }

    pub fn from_api(value: &str) -> Self {
        match value {
            "top" => Self::Top,
            "new" => Self::New,
            "controversial" => Self::Controversial,
            "old" => Self::Old,
            _ => Self::Best,
        }
    }
}

/// A comment node. Replies are stored as a tree, never flattened.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub created_utc: i64,
    #[serde(default)]
    pub distinguished: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub removed: bool,
    /// Reddit's "more comments" stub.
    #[serde(default)]
    pub is_more: bool,
    #[serde(default)]
    pub more_count: u32,
    #[serde(default)]
    pub more_children: Vec<String>,
    #[serde(default)]
    pub replies: Vec<Comment>,
}

impl Comment {
    pub fn new(id: impl Into<String>, author: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            author: author.into(),
            body: body.into(),
            ..Self::default()
        }
    }

    pub fn more(id: impl Into<String>, count: u32, children: Vec<String>) -> Self {
        Self {
            id: id.into(),
            is_more: true,
            more_count: count,
            more_children: children,
            author: "[more]".into(),
            ..Self::default()
        }
    }

    pub fn is_placeholder(&self) -> bool {
        self.deleted || self.removed || self.author == "[deleted]" || self.body == "[deleted]"
    }

    /// Number of descendants, excluding this node.
    pub fn descendant_count(&self) -> usize {
        self.replies.iter().map(|r| 1 + r.descendant_count()).sum()
    }

    /// Sort the entire subtree. More-comment stubs always sort last.
    pub fn sort_tree(&mut self, sort: CommentSort) {
        self.sort_children(sort);
        for reply in &mut self.replies {
            reply.sort_tree(sort);
        }
    }

    fn sort_children(&mut self, sort: CommentSort) {
        self.replies.sort_by(|a, b| {
            if a.is_more != b.is_more {
                return a.is_more.cmp(&b.is_more);
            }
            match sort {
                CommentSort::Best => b
                    .score
                    .cmp(&a.score)
                    .then_with(|| b.created_utc.cmp(&a.created_utc)),
                CommentSort::Top => b.score.cmp(&a.score),
                CommentSort::New => b.created_utc.cmp(&a.created_utc),
                CommentSort::Old => a.created_utc.cmp(&b.created_utc),
                CommentSort::Controversial => {
                    let ak = a.score.unsigned_abs() + a.descendant_count() as u64;
                    let bk = b.score.unsigned_abs() + b.descendant_count() as u64;
                    ak.cmp(&bk)
                        .then_with(|| b.replies.len().cmp(&a.replies.len()))
                }
            }
        });
    }

    /// Recursively find a comment by id.
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Comment> {
        if self.id == id {
            return Some(self);
        }
        for reply in &mut self.replies {
            if let Some(found) = reply.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find(&self, id: &str) -> Option<&Comment> {
        if self.id == id {
            return Some(self);
        }
        self.replies.iter().find_map(|reply| reply.find(id))
    }

    /// Remove a "more" stub and splice loaded replies into its position.
    pub fn replace_more(&mut self, more_id: &str, loaded: Vec<Comment>) -> bool {
        if let Some(index) = self.replies.iter().position(|r| r.id == more_id) {
            self.replies.splice(index..=index, loaded);
            return true;
        }
        for reply in &mut self.replies {
            if reply.replace_more(more_id, loaded.clone()) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Comment {
        let mut root = Comment::new("a", "alice", "root");
        root.score = 10;
        let mut child = Comment::new("b", "bob", "child");
        child.score = 50;
        let mut second = Comment::new("c", "carol", "second");
        second.score = 1;
        root.replies = vec![child, second];
        root
    }

    #[test]
    fn descendant_count_is_recursive() {
        let mut root = tree();
        root.replies[0]
            .replies
            .push(Comment::new("d", "dan", "deep"));
        assert_eq!(root.descendant_count(), 3);
    }

    #[test]
    fn sorting_by_top_orders_children() {
        let mut root = tree();
        root.sort_tree(CommentSort::Top);
        assert_eq!(
            root.replies
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "c"]
        );
    }

    #[test]
    fn more_nodes_sort_last() {
        let mut root = tree();
        root.replies.push(Comment::more("more", 5, vec![]));
        root.sort_tree(CommentSort::Best);
        assert!(root.replies.last().unwrap().is_more);
    }

    #[test]
    fn replace_more_splices_replies() {
        let mut root = Comment::new("a", "a", "root");
        root.replies.push(Comment::more("m", 1, vec!["z".into()]));
        let loaded = vec![Comment::new("z", "zed", "loaded")];
        assert!(root.replace_more("m", loaded));
        assert_eq!(root.replies.len(), 1);
        assert_eq!(root.replies[0].id, "z");
    }
}
