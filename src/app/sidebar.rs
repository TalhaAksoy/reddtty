use crate::config::Settings;
use crate::models::FeedSpec;
use crate::providers::ProviderCapabilities;

#[derive(Clone, Debug)]
pub struct SidebarEntry {
    pub label: String,
    pub spec: FeedSpec,
    pub favorite: bool,
}

#[derive(Clone, Debug)]
pub struct SidebarSection {
    pub title: &'static str,
    pub entries: Vec<SidebarEntry>,
    pub empty_hint: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct SidebarState {
    pub selected: usize,
}

pub const SUGGESTED: [&str; 10] = [
    "rust",
    "linux",
    "programming",
    "technology",
    "opensource",
    "unixporn",
    "webdev",
    "machinelearning",
    "askscience",
    "gaming",
];

pub fn sections(settings: &Settings, caps: ProviderCapabilities) -> Vec<SidebarSection> {
    let mut out = Vec::new();

    let nav = vec![
        SidebarEntry {
            label: "Home".into(),
            spec: FeedSpec::Home,
            favorite: false,
        },
        SidebarEntry {
            label: "Popular".into(),
            spec: FeedSpec::Popular,
            favorite: false,
        },
    ];
    let _ = caps.saved; // Saved is not implemented by either provider yet.
    out.push(SidebarSection {
        title: "NAVIGATION",
        entries: nav,
        empty_hint: "",
    });

    let favorites: Vec<String> = settings.favorites.clone();
    let communities: Vec<SidebarEntry> = SUGGESTED
        .iter()
        .filter(|name| !favorites.iter().any(|f| f.eq_ignore_ascii_case(name)))
        .map(|name| subreddit_entry(name, false))
        .collect();
    if !communities.is_empty() {
        out.push(SidebarSection {
            title: "COMMUNITIES",
            entries: communities,
            empty_hint: "",
        });
    }

    out.push(SidebarSection {
        title: "FAVORITES",
        entries: favorites
            .iter()
            .map(|name| subreddit_entry(name, true))
            .collect(),
        empty_hint: "f to add",
    });

    let recent: Vec<SidebarEntry> = settings
        .recent
        .iter()
        .filter(|name| !favorites.iter().any(|f| f.eq_ignore_ascii_case(name)))
        .map(|name| subreddit_entry(name, false))
        .collect();
    if !recent.is_empty() {
        out.push(SidebarSection {
            title: "RECENT",
            entries: recent,
            empty_hint: "",
        });
    }

    out
}

fn subreddit_entry(name: &str, favorite: bool) -> SidebarEntry {
    SidebarEntry {
        label: format!("r/{name}"),
        spec: FeedSpec::Subreddit(name.to_string()),
        favorite,
    }
}

pub fn entries(settings: &Settings, caps: ProviderCapabilities) -> Vec<SidebarEntry> {
    sections(settings, caps)
        .into_iter()
        .flat_map(|section| section.entries)
        .collect()
}

pub fn clamp_selection(selected: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        selected.min(total - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_include_navigation_and_favorites() {
        let settings = Settings::default();
        let sections = sections(&settings, ProviderCapabilities::default());
        assert_eq!(sections[0].title, "NAVIGATION");
        assert!(sections.iter().any(|s| s.title == "FAVORITES"));
        let flat = entries(&settings, ProviderCapabilities::default());
        assert!(flat.iter().any(|e| e.spec == FeedSpec::Home));
        let favorites = sections
            .iter()
            .find(|section| section.title == "FAVORITES")
            .expect("favorites heading");
        assert!(favorites.entries.is_empty());
        assert_eq!(favorites.empty_hint, "f to add");
    }

    #[test]
    fn favorite_communities_are_not_duplicated() {
        let settings = Settings::default();
        let flat = entries(&settings, ProviderCapabilities::default());
        let rust_count = flat
            .iter()
            .filter(|e| e.spec == FeedSpec::Subreddit("rust".into()))
            .count();
        assert_eq!(rust_count, 1);
    }

    #[test]
    fn selection_is_clamped() {
        assert_eq!(clamp_selection(5, 3), 2);
        assert_eq!(clamp_selection(0, 0), 0);
    }
}
