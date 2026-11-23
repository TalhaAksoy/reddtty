use crate::models::{CommentSort, FeedSort, SearchScope, TopRange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortContext {
    Feed,
    Comments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortChoice {
    Feed(FeedSort),
    Comment(CommentSort),
    Range,
}

#[derive(Clone, Debug)]
pub struct SortModal {
    pub context: SortContext,
    pub choices: Vec<SortChoice>,
    pub selected: usize,
    pub top_range: TopRange,
}

impl SortModal {
    pub fn feed(sorts: Vec<FeedSort>, current: FeedSort, top_range: TopRange) -> Self {
        let mut choices: Vec<SortChoice> = sorts.into_iter().map(SortChoice::Feed).collect();
        let selected = choices
            .iter()
            .position(|c| matches!(c, SortChoice::Feed(s) if *s == current))
            .unwrap_or(0);
        choices.push(SortChoice::Range);
        Self {
            context: SortContext::Feed,
            choices,
            selected,
            top_range,
        }
    }

    pub fn comments(sorts: Vec<CommentSort>, current: CommentSort) -> Self {
        let choices: Vec<SortChoice> = sorts.into_iter().map(SortChoice::Comment).collect();
        let selected = choices
            .iter()
            .position(|c| matches!(c, SortChoice::Comment(s) if *s == current))
            .unwrap_or(0);
        Self {
            context: SortContext::Comments,
            choices,
            selected,
            top_range: TopRange::Week,
        }
    }

    pub fn len(&self) -> usize {
        self.choices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.choices.is_empty()
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.choices.is_empty() {
            return;
        }
        let max = self.choices.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
    }

    pub fn current(&self) -> Option<SortChoice> {
        self.choices.get(self.selected).copied()
    }

    pub fn is_range(&self) -> bool {
        self.current() == Some(SortChoice::Range)
    }

    pub fn cycle_range(&mut self, forward: bool) {
        let ranges = TopRange::ALL;
        let index = ranges
            .iter()
            .position(|r| *r == self.top_range)
            .unwrap_or(2);
        let next = if forward {
            (index + 1) % ranges.len()
        } else {
            (index + ranges.len() - 1) % ranges.len()
        };
        self.top_range = ranges[next];
    }

    pub fn label(&self, index: usize) -> String {
        match self.choices.get(index) {
            Some(SortChoice::Feed(sort)) => sort.label().to_string(),
            Some(SortChoice::Comment(sort)) => sort.label().to_string(),
            Some(SortChoice::Range) => {
                format!("Top time range · {}", self.top_range.label())
            }
            None => String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RangeModal {
    pub selected: usize,
}

impl RangeModal {
    pub fn current(top_range: TopRange) -> Self {
        let selected = TopRange::ALL
            .iter()
            .position(|r| *r == top_range)
            .unwrap_or(2);
        Self { selected }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let max = TopRange::ALL.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
    }

    pub fn value(&self) -> TopRange {
        TopRange::ALL[self.selected.min(TopRange::ALL.len() - 1)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    GoToSubreddit,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScopeChoice {
    All,
    Current,
}

#[derive(Clone, Debug)]
pub struct TextModal {
    pub kind: TextKind,
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub error: Option<String>,
    pub scope: SearchScopeChoice,
    pub current_subreddit: Option<String>,
    pub cursor: usize,
}

impl TextModal {
    pub fn goto() -> Self {
        Self {
            kind: TextKind::GoToSubreddit,
            title: "Go to subreddit".into(),
            prompt: "r/".into(),
            value: String::new(),
            error: None,
            scope: SearchScopeChoice::All,
            current_subreddit: None,
            cursor: 0,
        }
    }

    pub fn search(current_subreddit: Option<String>) -> Self {
        let scope = if current_subreddit.is_some() {
            SearchScopeChoice::Current
        } else {
            SearchScopeChoice::All
        };
        Self {
            kind: TextKind::Search,
            title: "Search".into(),
            prompt: String::new(),
            value: String::new(),
            error: None,
            scope,
            current_subreddit,
            cursor: 0,
        }
    }

    pub fn push_char(&mut self, c: char) {
        if c.is_control() || self.value.chars().count() >= 120 {
            return;
        }
        let byte = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert(byte, c);
        self.cursor += 1;
        self.error = None;
    }

    pub fn paste(&mut self, text: &str) {
        for c in text.chars() {
            if !c.is_control() {
                self.push_char(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self
            .value
            .char_indices()
            .nth(self.cursor - 1)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
        self.error = None;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        let end = self
            .value
            .char_indices()
            .nth(self.cursor + 1)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.replace_range(start..end, "");
        self.error = None;
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let max = self.value.chars().count();
        self.cursor = if delta < 0 {
            self.cursor.saturating_sub(delta.unsigned_abs())
        } else {
            (self.cursor + delta as usize).min(max)
        };
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
        self.error = None;
    }

    pub fn toggle_scope(&mut self) {
        self.scope = match self.scope {
            SearchScopeChoice::All if self.current_subreddit.is_some() => {
                SearchScopeChoice::Current
            }
            _ => SearchScopeChoice::All,
        };
    }

    pub fn search_scope(&self) -> SearchScope {
        match self.scope {
            SearchScopeChoice::All => SearchScope::All,
            SearchScopeChoice::Current => self
                .current_subreddit
                .clone()
                .map(SearchScope::Subreddit)
                .unwrap_or(SearchScope::All),
        }
    }

    pub fn scope_label(&self) -> String {
        match self.search_scope() {
            SearchScope::All => "All Reddit".to_string(),
            SearchScope::Subreddit(name) => format!("r/{name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConfirmAction {
    RemoveFavorite(String),
}

#[derive(Clone, Debug)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

impl ConfirmModal {
    pub fn remove_favorite(name: &str) -> Self {
        Self {
            title: "Remove favorite".into(),
            message: format!("Remove r/{name} from favorites?"),
            action: ConfirmAction::RemoveFavorite(name.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SettingsModal {
    pub selected: usize,
    pub theme: crate::config::ThemeMode,
}

impl SettingsModal {
    pub fn new(theme: crate::config::ThemeMode) -> Self {
        Self { selected: 0, theme }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigPickerModal {
    pub selected: usize,
}

#[derive(Clone, Debug)]
pub enum Modal {
    Help,
    Sort(SortModal),
    Range(RangeModal),
    Text(TextModal),
    Confirm(ConfirmModal),
    Settings(SettingsModal),
    ConfigPicker(ConfigPickerModal),
}

impl Modal {
    pub fn title(&self) -> &str {
        match self {
            Self::Help => "Keyboard shortcuts",
            Self::Sort(modal) => match modal.context {
                SortContext::Feed => "Sort feed",
                SortContext::Comments => "Sort comments",
            },
            Self::Range(_) => "Top time range",
            Self::Text(modal) => &modal.title,
            Self::Confirm(modal) => &modal.title,
            Self::Settings(_) => "Settings",
            Self::ConfigPicker(_) => "Choose config",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_modal_starts_on_current_sort() {
        let modal = SortModal::feed(
            vec![FeedSort::Best, FeedSort::Hot, FeedSort::New, FeedSort::Top],
            FeedSort::New,
            TopRange::Week,
        );
        assert_eq!(modal.current(), Some(SortChoice::Feed(FeedSort::New)));
        assert_eq!(modal.len(), 5);
        assert_eq!(modal.label(4), "Top time range · This week");
    }

    #[test]
    fn range_cycling_wraps() {
        let mut modal = SortModal::feed(vec![FeedSort::Top], FeedSort::Top, TopRange::All);
        modal.cycle_range(true);
        assert_eq!(modal.top_range, TopRange::Hour);
        modal.cycle_range(false);
        assert_eq!(modal.top_range, TopRange::All);
    }

    #[test]
    fn text_input_edits_by_character_index() {
        let mut modal = TextModal::goto();
        for c in "rüst".chars() {
            modal.push_char(c);
        }
        assert_eq!(modal.value, "rüst");
        modal.backspace();
        assert_eq!(modal.value, "rüs");
        modal.move_cursor(-2);
        modal.delete();
        assert_eq!(modal.value, "rs");
    }

    #[test]
    fn search_scope_defaults_to_current_and_toggles() {
        let mut modal = TextModal::search(Some("rust".into()));
        assert_eq!(modal.scope_label(), "r/rust");
        modal.toggle_scope();
        assert_eq!(modal.scope_label(), "All Reddit");
    }

    #[test]
    fn range_modal_value_is_clamped() {
        let mut modal = RangeModal::current(TopRange::All);
        modal.move_selection(100);
        assert_eq!(modal.value(), TopRange::All);
    }
}
