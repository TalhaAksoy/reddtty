use crate::models::{CommentSort, FeedSort, SearchScope, TopRange};

use super::help::HelpModal;

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
    AddFilter,
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

    pub fn add_filter() -> Self {
        Self {
            kind: TextKind::AddFilter,
            title: "Add local filter".into(),
            prompt: String::new(),
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
pub struct ThemePickerModal {
    pub selected: usize,
    pub original: crate::config::ThemeMode,
}

impl ThemePickerModal {
    pub fn new(current: crate::config::ThemeMode) -> Self {
        let selected = crate::config::ThemeMode::ALL
            .iter()
            .position(|theme| *theme == current)
            .unwrap_or(0);
        Self {
            selected,
            original: current,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BookmarksModal {
    pub selected: usize,
}

impl BookmarksModal {
    pub fn clamp(&mut self, total: usize) {
        self.selected = self.selected.min(total.saturating_sub(1));
    }
}

#[derive(Clone, Debug, Default)]
pub struct InfoSnapshot {
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub members: Option<(u64, u64)>,
    pub error: Option<String>,
    pub loading: bool,
    pub post_lines: Vec<String>,
    pub actions: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct InfoModal {
    pub info: InfoSnapshot,
}

#[derive(Clone, Debug)]
pub struct ImageProtocolModal {
    pub selected: usize,
}

impl ImageProtocolModal {
    pub fn new(current: crate::config::ImageProtocolSetting) -> Self {
        let selected = crate::config::ImageProtocolSetting::ALL
            .iter()
            .position(|setting| *setting == current)
            .unwrap_or(0);
        Self { selected }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let max = crate::config::ImageProtocolSetting::ALL.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
    }

    pub fn current(&self) -> crate::config::ImageProtocolSetting {
        crate::config::ImageProtocolSetting::ALL
            .get(self.selected)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppearanceModal {
    pub selected: usize,
}

impl AppearanceModal {
    pub const ROWS: usize = 2;
}

#[derive(Clone, Debug, Default)]
pub struct FiltersModal {
    pub selected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandId {
    Help,
    Settings,
    Filters,
    SourceHealth,
    Info,
    Bookmarks,
    Search,
    GoToSubreddit,
    ChangeSource,
    Refresh,
    ToggleSidebar,
    SortBest,
    SortHot,
    SortNew,
    SortTop,
    RangeAll,
    ThemeNext,
    ThemeDark,
    ThemeLight,
    ThemeLatte,
    ThemeFrappe,
    ThemeMacchiato,
    ThemeMocha,
    ThemeHighContrast,
    ThemeMono,
    CopyPermalink,
    CopyImageUrl,
    OpenComments,
    OpenSubreddit,
    MarkRead,
    AddFilter,
}

impl CommandId {
    pub const ALL: [Self; 31] = [
        Self::Search,
        Self::GoToSubreddit,
        Self::SortBest,
        Self::SortHot,
        Self::SortNew,
        Self::SortTop,
        Self::RangeAll,
        Self::ThemeNext,
        Self::ThemeDark,
        Self::ThemeLight,
        Self::ThemeLatte,
        Self::ThemeFrappe,
        Self::ThemeMacchiato,
        Self::ThemeMocha,
        Self::ThemeHighContrast,
        Self::ThemeMono,
        Self::Bookmarks,
        Self::MarkRead,
        Self::CopyPermalink,
        Self::CopyImageUrl,
        Self::OpenComments,
        Self::OpenSubreddit,
        Self::Filters,
        Self::AddFilter,
        Self::SourceHealth,
        Self::Info,
        Self::ChangeSource,
        Self::Refresh,
        Self::ToggleSidebar,
        Self::Settings,
        Self::Help,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Help => "Keyboard shortcuts",
            Self::Settings => "Open settings",
            Self::Filters => "Open local filters",
            Self::SourceHealth => "Open source health",
            Self::Info => "Show community info",
            Self::Bookmarks => "Open bookmarks",
            Self::Search => "Search posts",
            Self::GoToSubreddit => "Go to subreddit",
            Self::ChangeSource => "Change source / config",
            Self::Refresh => "Refresh current view",
            Self::ToggleSidebar => "Toggle sidebar",
            Self::SortBest => "Sort feed: Best",
            Self::SortHot => "Sort feed: Hot",
            Self::SortNew => "Sort feed: New",
            Self::SortTop => "Sort feed: Top",
            Self::RangeAll => "Top range: All time",
            Self::ThemeNext => "Theme: next",
            Self::ThemeDark => "Theme: dark",
            Self::ThemeLight => "Theme: light",
            Self::ThemeLatte => "Theme: Catppuccin Latte",
            Self::ThemeFrappe => "Theme: Catppuccin Frappé",
            Self::ThemeMacchiato => "Theme: Catppuccin Macchiato",
            Self::ThemeMocha => "Theme: Catppuccin Mocha",
            Self::ThemeHighContrast => "Theme: high contrast",
            Self::ThemeMono => "Theme: monochrome",
            Self::CopyPermalink => "Copy permalink",
            Self::CopyImageUrl => "Copy image URL",
            Self::OpenComments => "Open comments in browser",
            Self::OpenSubreddit => "Open subreddit in browser",
            Self::MarkRead => "Toggle read state",
            Self::AddFilter => "Add local filter",
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Self::Help => "keys shortcuts help ?",
            Self::Settings => "config preferences theme settings S",
            Self::Filters => "hide read keyword mute sub author domain",
            Self::SourceHealth => "provider latency status error retry source",
            Self::Info => "community about description members sidebar i",
            Self::Bookmarks => "saved read later local B",
            Self::Search => "find query /",
            Self::GoToSubreddit => "community r/ g",
            Self::ChangeSource => "provider config switch redlib demo",
            Self::Refresh => "reload r",
            Self::ToggleSidebar => "hide show sidebar h",
            Self::SortBest => "order best",
            Self::SortHot => "order hot",
            Self::SortNew => "order new",
            Self::SortTop => "order top",
            Self::RangeAll => "top all time range",
            Self::ThemeNext => "cycle theme T",
            Self::ThemeDark => "colors dark",
            Self::ThemeLight => "colors light",
            Self::ThemeLatte => "colors catppuccin latte",
            Self::ThemeFrappe => "colors catppuccin frappe",
            Self::ThemeMacchiato => "colors catppuccin macchiato",
            Self::ThemeMocha => "colors catppuccin mocha",
            Self::ThemeHighContrast => "colors high contrast accessibility",
            Self::ThemeMono => "colors monochrome mono no color",
            Self::CopyPermalink => "clipboard link y",
            Self::CopyImageUrl => "clipboard image url picture",
            Self::OpenComments => "browser thread comments Y",
            Self::OpenSubreddit => "browser community subreddit G",
            Self::MarkRead => "unread read m",
            Self::AddFilter => "filter keyword mute",
        }
    }

    pub fn matches(self, query: &str) -> bool {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return true;
        }
        let haystack = format!("{} {}", self.label(), self.keywords()).to_ascii_lowercase();
        query.split_whitespace().all(|part| haystack.contains(part))
    }
}

#[derive(Clone, Debug, Default)]
pub struct CommandPaletteModal {
    pub query: String,
    pub selected: usize,
    pub recent: Vec<CommandId>,
    pub error: Option<String>,
}

impl CommandPaletteModal {
    pub fn filtered(&self) -> Vec<CommandId> {
        if self.query.trim().is_empty() {
            let mut out = Vec::new();
            for command in &self.recent {
                if !out.contains(command) {
                    out.push(*command);
                }
            }
            for command in CommandId::ALL {
                if !out.contains(&command) {
                    out.push(command);
                }
            }
            return out;
        }
        CommandId::ALL
            .into_iter()
            .filter(|command| command.matches(&self.query))
            .collect()
    }

    pub fn selected_command(&self) -> Option<CommandId> {
        self.filtered().get(self.selected).copied()
    }

    pub fn move_selection(&mut self, delta: isize) {
        let total = self.filtered().len();
        if total == 0 {
            self.selected = 0;
            return;
        }
        let max = total - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
    }

    pub fn push_char(&mut self, c: char) {
        if c.is_control() || self.query.chars().count() >= 80 {
            return;
        }
        self.query.push(c);
        self.selected = 0;
        self.error = None;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
        self.error = None;
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.error = None;
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceModal {
    pub selected: usize,
}

impl SourceModal {
    pub const ACTIONS: [&'static str; 2] = ["Test now", "Change source"];

    pub fn move_selection(&mut self, delta: isize) {
        let max = Self::ACTIONS.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(max)
        };
    }
}

#[derive(Clone, Debug)]
pub enum Modal {
    Help(HelpModal),
    Sort(SortModal),
    Range(RangeModal),
    Text(TextModal),
    Confirm(ConfirmModal),
    Settings(SettingsModal),
    ConfigPicker(ConfigPickerModal),
    ThemePicker(ThemePickerModal),
    Bookmarks(BookmarksModal),
    Source(SourceModal),
    Filters(FiltersModal),
    Info(InfoModal),
    ImagePicker(ImageProtocolModal),
    Appearance(AppearanceModal),
    CommandPalette(CommandPaletteModal),
}

impl Modal {
    pub fn title(&self) -> &str {
        match self {
            Self::Help(_) => "Keyboard shortcuts",
            Self::Sort(modal) => match modal.context {
                SortContext::Feed => "Sort feed",
                SortContext::Comments => "Sort comments",
            },
            Self::Range(_) => "Top time range",
            Self::Text(modal) => &modal.title,
            Self::Confirm(modal) => &modal.title,
            Self::Settings(_) => "Settings",
            Self::ConfigPicker(_) => "Choose config",
            Self::ThemePicker(_) => "Theme",
            Self::Bookmarks(_) => "Bookmarks",
            Self::Source(_) => "Source health",
            Self::Filters(_) => "Local filters",
            Self::Info(_) => "Info",
            Self::ImagePicker(_) => "Image protocol",
            Self::Appearance(_) => "Appearance",
            Self::CommandPalette(_) => "Commands",
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
    fn command_palette_filters_by_label_and_keywords() {
        let mut modal = CommandPaletteModal::default();
        assert_eq!(modal.filtered().len(), CommandId::ALL.len());
        assert!(CommandId::ALL.contains(&CommandId::CopyImageUrl));
        assert!(CommandId::ALL.contains(&CommandId::OpenComments));
        assert!(CommandId::ALL.contains(&CommandId::OpenSubreddit));
        for c in "mocha".chars() {
            modal.push_char(c);
        }
        assert_eq!(modal.filtered(), vec![CommandId::ThemeMocha]);
        assert_eq!(modal.selected_command(), Some(CommandId::ThemeMocha));
        modal.clear();
        for c in "book".chars() {
            modal.push_char(c);
        }
        assert_eq!(modal.selected_command(), Some(CommandId::Bookmarks));
        modal.clear();
        for c in "monochrome".chars() {
            modal.push_char(c);
        }
        assert_eq!(modal.selected_command(), Some(CommandId::ThemeMono));
        modal.clear();
        for c in "contrast".chars() {
            modal.push_char(c);
        }
        assert_eq!(modal.selected_command(), Some(CommandId::ThemeHighContrast));
        modal.clear();
        for c in "book".chars() {
            modal.push_char(c);
        }
        modal.push_char('z');
        assert!(modal.filtered().is_empty());
        modal.backspace();
        assert_eq!(modal.selected_command(), Some(CommandId::Bookmarks));
    }

    #[test]
    fn command_palette_moves_selection_within_results() {
        let mut modal = CommandPaletteModal::default();
        modal.move_selection(3);
        assert_eq!(modal.selected, 3);
        modal.move_selection(-100);
        assert_eq!(modal.selected, 0);
        modal.move_selection(10_000);
        assert_eq!(modal.selected, CommandId::ALL.len() - 1);
    }

    #[test]
    fn range_modal_value_is_clamped() {
        let mut modal = RangeModal::current(TopRange::All);
        modal.move_selection(100);
        assert_eq!(modal.value(), TopRange::All);
    }
}
