//! Categorised keyboard-help content and scroll state.
//!
//! The help modal is a real reference, not a flat shortcut dump: sections are
//! grouped by the screen/pane where the keys work, and the content scrolls on
//! terminals that cannot show everything at once.

pub struct HelpEntry {
    pub keys: &'static str,
    pub desc: &'static str,
}

pub struct HelpSection {
    pub title: &'static str,
    pub entries: &'static [HelpEntry],
}

const fn entry(keys: &'static str, desc: &'static str) -> HelpEntry {
    HelpEntry { keys, desc }
}

pub const HELP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "GLOBAL",
        entries: &[
            entry("?", "Open this help"),
            entry("S", "Settings"),
            entry(": / Ctrl+P", "Command palette"),
            entry("T", "Cycle theme"),
            entry("Esc", "Back / close modal"),
            entry("Ctrl+C", "Quit immediately"),
            entry("q", "Quit (not in text input)"),
        ],
    },
    HelpSection {
        title: "FEED",
        entries: &[
            entry("j / k", "Next / previous post"),
            entry("↑ / ↓", "Same as j / k"),
            entry("J / K", "Scroll one line inside a large card"),
            entry("←", "Focus sidebar (shows it if wide)"),
            entry("→", "Return focus from sidebar to feed"),
            entry("PgUp / PgDn", "Move by half a viewport"),
            entry("Home / End", "First / last loaded post"),
            entry("Enter", "Open selected post"),
            entry("s", "Feed sort menu"),
            entry("t", "Top time range"),
            entry("g", "Go to subreddit"),
            entry("/", "Search posts"),
            entry("r", "Refresh first page"),
            entry("R", "Retry a failed “load more” request"),
            entry("o", "Open post/link in browser"),
            entry("v", "Open image / gallery"),
            entry("x", "Reveal NSFW or spoiler"),
            entry("f", "Favorite subreddit"),
            entry("b", "Bookmark selected post"),
            entry("B", "Open bookmarks"),
            entry("m", "Toggle read / unread"),
            entry("y", "Copy post permalink"),
            entry("i", "Community info drawer"),
            entry("I", "Copy image preview URL"),
            entry("M / A / D", "Mute subreddit / author / domain"),
            entry("u", "Undo the last quick mute"),
            entry("Y", "Open comments in browser"),
            entry("G", "Open subreddit in browser"),
            entry("h", "Show / hide sidebar"),
            entry("Tab", "Focus sidebar / feed"),
        ],
    },
    HelpSection {
        title: "SIDEBAR",
        entries: &[
            entry("j / k", "Move between entries"),
            entry("Enter", "Open selected entry"),
            entry("→", "Return focus to feed"),
            entry("Esc", "Return focus to feed"),
            entry("Tab", "Return focus to feed"),
            entry("f", "Add/remove favorite"),
            entry("h", "Hide sidebar"),
            entry("G", "Open selected subreddit"),
            entry("s, t, g, /", "Same feed actions"),
            entry("r, o, v, x", "Same feed actions"),
        ],
    },
    HelpSection {
        title: "POST DETAIL",
        entries: &[
            entry("Tab", "Switch body / comments"),
            entry("← / →", "Focus body / comments"),
            entry("j / k", "Scroll body or next comment"),
            entry("PgUp / PgDn", "Page focused pane"),
            entry("Home / End", "Start / end of pane"),
            entry("Enter", "Open image or toggle comment"),
            entry("c", "Toggle selected comment"),
            entry("[ / ]", "Previous / next root comment"),
            entry("p", "Jump to parent comment"),
            entry("z", "Collapse / expand all threads"),
            entry("O", "Only original poster's comments"),
            entry("n", "Next unread comment"),
            entry("y", "Copy comment permalink"),
            entry("i", "Community info drawer"),
            entry("I", "Copy post image URL"),
            entry("M / A / D", "Mute subreddit / author / domain"),
            entry("u", "Undo the last quick mute"),
            entry("Y", "Open permalink in browser"),
            entry("G", "Open subreddit in browser"),
            entry("s", "Comment sort menu"),
            entry("v", "Open image / gallery"),
            entry("o", "Open post/link in browser"),
            entry("x", "Reveal NSFW or spoiler"),
            entry("r", "Refresh post and comments"),
            entry("b / B", "Bookmark / bookmarks list"),
            entry("m", "Toggle post read state"),
            entry("Esc", "Back to feed"),
        ],
    },
    HelpSection {
        title: "IMAGE VIEWER",
        entries: &[
            entry("← / h", "Previous image"),
            entry("→ / l", "Next image"),
            entry("o", "Open image externally"),
            entry("y / I", "Copy image URL"),
            entry("i", "Community info drawer"),
            entry("Y", "Open comments in browser"),
            entry("G", "Open subreddit in browser"),
            entry("+ / -", "Zoom in / out"),
            entry("Shift+arrows", "Pan a zoomed image"),
            entry("0", "Reset zoom and pan"),
            entry("Home / End", "First / last image"),
            entry("Click thumbnail", "Jump to that image"),
            entry("r", "Clear and redraw image"),
            entry("T", "Cycle theme"),
            entry("Esc", "Close viewer"),
        ],
    },
    HelpSection {
        title: "CLIPBOARD & LINKS",
        entries: &[
            entry("y", "Copy post/comment permalink"),
            entry("I", "Copy image preview URL"),
            entry("y / I (viewer)", "Copy current image URL"),
            entry("Y", "Open comments/permalink"),
            entry("G", "Open subreddit"),
            entry("o", "Open post/image externally"),
            entry("b / B", "Bookmarks stay local"),
        ],
    },
    HelpSection {
        title: "SORT & RANGE",
        entries: &[
            entry("j / k", "Move selection"),
            entry("Enter", "Apply sort or open range"),
            entry("h / l", "Cycle Top range on range row"),
            entry("Esc", "Close without applying"),
        ],
    },
    HelpSection {
        title: "TEXT INPUT",
        entries: &[
            entry("Characters", "Insert text"),
            entry("Backspace", "Delete before cursor"),
            entry("Delete", "Delete at cursor"),
            entry("← / →", "Move cursor"),
            entry("Home / End", "Start / end of input"),
            entry("Ctrl+U", "Clear input"),
            entry("Tab", "Search scope"),
            entry("Enter", "Submit"),
            entry("Esc", "Cancel / back"),
            entry("Paste", "Insert pasted text"),
        ],
    },
    HelpSection {
        title: "BOOKMARKS",
        entries: &[
            entry("j / k", "Move between saved posts"),
            entry("Enter", "Open saved post"),
            entry("d / Delete", "Remove selected bookmark"),
            entry("Esc", "Close list"),
            entry("b", "Save from feed/details"),
        ],
    },
    HelpSection {
        title: "LOCAL FILTERS",
        entries: &[
            entry("j / k", "Move between rules"),
            entry("Enter", "Toggle hide-read row"),
            entry("a", "Add a filter rule"),
            entry("d / Delete", "Remove selected rule"),
            entry("C", "Clear all filters"),
            entry("Esc", "Back to Settings"),
            entry("keyword:", "Block title/body text"),
            entry("sub:", "Mute a subreddit"),
            entry("author:", "Mute a username"),
            entry("domain:", "Mute a link domain"),
        ],
    },
    HelpSection {
        title: "SOURCE HEALTH",
        entries: &[
            entry("j / k", "Move between actions"),
            entry("Enter / r", "Test now"),
            entry("c", "Change source"),
            entry("Esc", "Back to Settings"),
        ],
    },
    HelpSection {
        title: "COMMAND PALETTE",
        entries: &[
            entry("Characters", "Filter commands"),
            entry("↑ / ↓", "Move selection"),
            entry("Tab", "Next result"),
            entry("Backspace", "Delete query character"),
            entry("Ctrl+U", "Clear query"),
            entry("Enter", "Run selected command"),
            entry("Esc", "Close palette"),
        ],
    },
    HelpSection {
        title: "MOUSE",
        entries: &[
            entry("Click card", "Select and focus feed"),
            entry("Double-click", "Open post"),
            entry("Click sidebar", "Open that feed"),
            entry("Click sort tab", "Apply that sort"),
            entry("Click comment", "Select and focus comments"),
            entry("Double-click", "Collapse / expand comment"),
            entry("Wheel", "Scroll focused pane"),
            entry("Gallery arrows", "Previous / next image"),
            entry("Outside modal", "Close modal"),
        ],
    },
];

pub const TWO_COLUMN_WIDTH: u16 = 110;

pub fn columns_for_width(inner_width: u16) -> usize {
    if inner_width >= TWO_COLUMN_WIDTH {
        2
    } else {
        1
    }
}

/// The modal is capped so it stays readable on very tall terminals.
pub fn modal_height(area_height: u16) -> u16 {
    area_height
        .saturating_sub(4)
        .clamp(8, 32)
        .min(area_height.saturating_sub(2))
}

pub fn visible_rows(area_height: u16) -> usize {
    modal_height(area_height).saturating_sub(3) as usize
}

pub fn total_rows(columns: usize) -> usize {
    let columns = columns.max(1);
    HELP_SECTIONS
        .iter()
        .map(|section| 1 + section.entries.len().div_ceil(columns) + 1)
        .sum::<usize>()
        .saturating_sub(1)
}

pub fn max_offset(width: u16, height: u16) -> usize {
    let rect_width = width.saturating_sub(4).min(120);
    let inner_width = rect_width.saturating_sub(4);
    let columns = columns_for_width(inner_width);
    total_rows(columns).saturating_sub(visible_rows(height).max(1))
}

pub struct HelpView {
    pub section: &'static HelpSection,
    pub entries: Vec<&'static HelpEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct HelpModal {
    /// First visible row; rendered content clamps this again defensively.
    pub offset: usize,
    pub query: String,
    pub searching: bool,
    /// Section title to put first when opened from a specific screen.
    pub context: Option<&'static str>,
}

impl HelpModal {
    pub fn visible_sections(&self) -> Vec<HelpView> {
        let query = self.query.trim().to_ascii_lowercase();
        let mut ordered: Vec<&'static HelpSection> = Vec::new();
        if query.is_empty() {
            if let Some(context) = self.context
                && let Some(section) = HELP_SECTIONS
                    .iter()
                    .find(|section| section.title == context)
            {
                ordered.push(section);
            }
            for section in HELP_SECTIONS {
                if !ordered
                    .iter()
                    .any(|existing| existing.title == section.title)
                {
                    ordered.push(section);
                }
            }
        } else {
            ordered.extend(HELP_SECTIONS);
        }

        let mut views = Vec::new();
        for section in ordered {
            let section_matches = section.title.to_ascii_lowercase().contains(&query);
            let entries: Vec<&'static HelpEntry> = if query.is_empty() {
                section.entries.iter().collect()
            } else {
                section
                    .entries
                    .iter()
                    .filter(|entry| {
                        section_matches
                            || entry.keys.to_ascii_lowercase().contains(&query)
                            || entry.desc.to_ascii_lowercase().contains(&query)
                    })
                    .collect()
            };
            if !entries.is_empty() {
                views.push(HelpView { section, entries });
            }
        }
        views
    }

    pub fn total_rows(&self, columns: usize) -> usize {
        let columns = columns.max(1);
        self.visible_sections()
            .iter()
            .map(|view| 1 + view.entries.len().div_ceil(columns) + 1)
            .sum::<usize>()
            .saturating_sub(1)
    }

    pub fn scroll_by(&mut self, delta: isize, width: u16, height: u16) {
        let max = self.max_offset(width, height) as i64;
        self.offset = (self.offset as i64 + delta as i64).clamp(0, max) as usize;
    }

    pub fn page(&mut self, down: bool, width: u16, height: u16) {
        let step = visible_rows(height).max(1) as isize;
        self.scroll_by(if down { step } else { -step }, width, height);
    }

    pub fn home(&mut self) {
        self.offset = 0;
    }

    pub fn end(&mut self, width: u16, height: u16) {
        self.offset = self.max_offset(width, height);
    }

    pub fn clamped(&self, width: u16, height: u16) -> usize {
        self.offset.min(self.max_offset(width, height))
    }

    pub fn max_offset(&self, width: u16, height: u16) -> usize {
        let rect_width = width.saturating_sub(4).min(120);
        let inner_width = rect_width.saturating_sub(4);
        let columns = columns_for_width(inner_width);
        self.total_rows(columns)
            .saturating_sub(visible_rows(height).max(1))
    }

    pub fn push_char(&mut self, c: char) {
        if c.is_control() || self.query.chars().count() >= 60 {
            return;
        }
        self.query.push(c);
        self.offset = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.offset = 0;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.offset = 0;
        self.searching = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_entries_and_unique_titles() {
        let mut titles = Vec::new();
        for section in HELP_SECTIONS {
            assert!(!section.entries.is_empty(), "{}", section.title);
            assert!(!titles.contains(&section.title), "{}", section.title);
            titles.push(section.title);
            for entry in section.entries {
                assert!(!entry.keys.is_empty(), "{}", section.title);
                assert!(!entry.desc.is_empty(), "{}", section.title);
            }
        }
        assert!(HELP_SECTIONS.len() >= 8);
        assert!(titles.contains(&"CLIPBOARD & LINKS"));
        assert!(titles.contains(&"IMAGE VIEWER"));
        assert!(titles.contains(&"COMMAND PALETTE"));
    }

    #[test]
    fn two_columns_need_fewer_rows() {
        assert!(columns_for_width(200) == 2);
        assert!(columns_for_width(80) == 1);
        assert!(total_rows(2) < total_rows(1));
    }

    #[test]
    fn scrolling_is_clamped_to_the_content() {
        let mut modal = HelpModal::default();
        modal.end(120, 30);
        assert_eq!(modal.offset, max_offset(120, 30));
        modal.scroll_by(10, 120, 30);
        assert_eq!(modal.offset, max_offset(120, 30));
        modal.home();
        assert_eq!(modal.offset, 0);
        modal.scroll_by(-10, 120, 30);
        assert_eq!(modal.offset, 0);
        modal.page(true, 120, 30);
        assert!(modal.offset > 0);
    }

    #[test]
    fn context_section_is_first_when_help_opens() {
        let modal = HelpModal {
            context: Some("POST DETAIL"),
            ..HelpModal::default()
        };
        let views = modal.visible_sections();
        assert_eq!(views[0].section.title, "POST DETAIL");
    }

    #[test]
    fn search_filters_entries_and_sections() {
        let modal = HelpModal {
            query: "zoom".into(),
            ..HelpModal::default()
        };
        let views = modal.visible_sections();
        assert!(
            views
                .iter()
                .any(|view| view.section.title == "IMAGE VIEWER")
        );
        assert!(
            views
                .iter()
                .flat_map(|view| &view.entries)
                .any(|entry| entry.desc.contains("Zoom"))
        );
        assert!(!views.iter().any(|view| view.section.title == "FEED"));
        assert!(modal.total_rows(1) < total_rows(1));
    }

    #[test]
    fn tall_terminals_cap_the_modal_height() {
        assert_eq!(modal_height(120), 32);
        assert_eq!(visible_rows(120), 29);
        assert!(max_offset(200, 120) > 0);
        assert!(max_offset(200, 120) < max_offset(80, 120));
    }
}
