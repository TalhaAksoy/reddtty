use serde::{Deserialize, Serialize};

use crate::models::Post;

/// Feed ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedSort {
    Best,
    Hot,
    New,
    Top,
    Rising,
    Controversial,
}

impl FeedSort {
    pub const ALL: [Self; 6] = [
        Self::Best,
        Self::Hot,
        Self::New,
        Self::Top,
        Self::Rising,
        Self::Controversial,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Best => "Best",
            Self::Hot => "Hot",
            Self::New => "New",
            Self::Top => "Top",
            Self::Rising => "Rising",
            Self::Controversial => "Controversial",
        }
    }

    /// Reddit listing path segment.
    pub fn api_value(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Hot => "hot",
            Self::New => "new",
            Self::Top => "top",
            Self::Rising => "rising",
            Self::Controversial => "controversial",
        }
    }

    pub fn api_value_for(self, spec: &FeedSpec) -> &'static str {
        if matches!(spec, FeedSpec::Search { .. }) {
            match self {
                Self::Best => "relevance",
                Self::Controversial => "comments",
                other => other.api_value(),
            }
        } else {
            self.api_value()
        }
    }

    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "best" | "relevance" => Some(Self::Best),
            "hot" => Some(Self::Hot),
            "new" => Some(Self::New),
            "top" => Some(Self::Top),
            "rising" => Some(Self::Rising),
            "controversial" | "comments" => Some(Self::Controversial),
            _ => None,
        }
    }
}

/// Time window used by `Top`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopRange {
    Hour,
    Day,
    Week,
    Month,
    Year,
    All,
}

impl TopRange {
    pub const ALL: [Self; 6] = [
        Self::Hour,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Year,
        Self::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hour => "Past hour",
            Self::Day => "Today",
            Self::Week => "This week",
            Self::Month => "This month",
            Self::Year => "This year",
            Self::All => "All time",
        }
    }

    pub fn api_value(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::All => "all",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            Self::Hour => 3_600,
            Self::Day => 86_400,
            Self::Week => 604_800,
            Self::Month => 2_592_000,
            Self::Year => 31_536_000,
            Self::All => i64::MAX,
        }
    }

    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hour" | "past hour" => Some(Self::Hour),
            "day" | "today" => Some(Self::Day),
            "week" | "this week" => Some(Self::Week),
            "month" | "this month" => Some(Self::Month),
            "year" | "this year" => Some(Self::Year),
            "all" | "all time" => Some(Self::All),
            _ => None,
        }
    }
}

/// Where a feed's content comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SearchScope {
    All,
    Subreddit(String),
}

impl SearchScope {
    pub fn label(&self) -> String {
        match self {
            Self::All => "All Reddit".to_string(),
            Self::Subreddit(name) => format!("r/{name}"),
        }
    }

    pub fn subreddit(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Subreddit(name) => Some(name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FeedSpec {
    Home,
    Popular,
    Subreddit(String),
    Search { query: String, scope: SearchScope },
}

impl FeedSpec {
    pub fn title(&self) -> String {
        match self {
            Self::Home => "Home".to_string(),
            Self::Popular => "Popular".to_string(),
            Self::Subreddit(name) => format!("r/{name}"),
            Self::Search { query, .. } => {
                if query.is_empty() {
                    "Search".to_string()
                } else {
                    format!("Search: {query}")
                }
            }
        }
    }

    pub fn short_title(&self) -> String {
        match self {
            Self::Home => "Home".to_string(),
            Self::Popular => "Popular".to_string(),
            Self::Subreddit(name) => format!("r/{name}"),
            Self::Search { query, .. } => format!("⌕ {query}"),
        }
    }

    pub fn subreddit_name(&self) -> Option<&str> {
        match self {
            Self::Subreddit(name) => Some(name),
            Self::Search { scope, .. } => scope.subreddit(),
            _ => None,
        }
    }

    pub fn is_search(&self) -> bool {
        matches!(self, Self::Search { .. })
    }

    /// Baseline capability rules for the real Reddit API. Providers may
    /// support more (the demo provider supports every sort explicitly).
    pub fn supports_sort(&self, sort: FeedSort) -> bool {
        match self {
            Self::Home => true,
            Self::Popular => matches!(sort, FeedSort::Hot | FeedSort::New | FeedSort::Top),
            Self::Subreddit(_) => sort != FeedSort::Best,
            Self::Search { .. } => matches!(
                sort,
                FeedSort::Best
                    | FeedSort::Hot
                    | FeedSort::New
                    | FeedSort::Top
                    | FeedSort::Controversial
            ),
        }
    }

    pub fn default_sort(&self) -> FeedSort {
        match self {
            Self::Home => FeedSort::Best,
            Self::Popular | Self::Subreddit(_) => FeedSort::Hot,
            Self::Search { .. } => FeedSort::Best,
        }
    }

    pub fn sort_options(&self) -> Vec<FeedSort> {
        FeedSort::ALL
            .into_iter()
            .filter(|s| self.supports_sort(*s))
            .collect()
    }
}

/// A single provider request. `after` is an opaque cursor, never a page number.
#[derive(Clone, Debug)]
pub struct FeedRequest {
    pub spec: FeedSpec,
    pub sort: FeedSort,
    pub top_range: TopRange,
    pub after: Option<String>,
    pub limit: u32,
}

impl FeedRequest {
    pub fn first(spec: FeedSpec, sort: FeedSort, top_range: TopRange, limit: u32) -> Self {
        Self {
            spec,
            sort,
            top_range,
            after: None,
            limit: limit.clamp(1, 100),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FeedPage {
    pub posts: Vec<Post>,
    pub after: Option<String>,
    pub has_more: bool,
}

impl FeedPage {
    pub fn is_empty(&self) -> bool {
        self.posts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_labels_and_api_values() {
        assert_eq!(FeedSort::Top.api_value(), "top");
        assert_eq!(
            FeedSort::Best.api_value_for(&FeedSpec::Search {
                query: "x".into(),
                scope: SearchScope::All
            }),
            "relevance"
        );
        assert_eq!(FeedSort::from_str_loose("NEW"), Some(FeedSort::New));
    }

    #[test]
    fn subreddit_does_not_support_best() {
        let spec = FeedSpec::Subreddit("rust".into());
        assert!(!spec.supports_sort(FeedSort::Best));
        assert!(spec.supports_sort(FeedSort::Top));
    }

    #[test]
    fn popular_has_limited_sorts() {
        let options = FeedSpec::Popular.sort_options();
        assert_eq!(options, vec![FeedSort::Hot, FeedSort::New, FeedSort::Top]);
    }

    #[test]
    fn top_ranges_are_ordered() {
        assert!(TopRange::Hour.seconds() < TopRange::Day.seconds());
        assert_eq!(TopRange::from_str_loose("this week"), Some(TopRange::Week));
    }
}
