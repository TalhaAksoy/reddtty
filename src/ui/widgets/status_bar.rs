use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::modal::TextKind;
use crate::app::{Focus, Modal, View};
use crate::ui::theme::Theme;
use crate::utils::text::width;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    view: Option<&View>,
    focus: Focus,
    modal: Option<&Modal>,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut pairs: Vec<(&str, &str)> = if modal.is_some() {
        match modal {
            Some(Modal::Help(_)) => {
                vec![("j/k", "Scroll"), ("PgUp/PgDn", "Page"), ("Esc/?", "Close")]
            }
            Some(Modal::Sort(_)) => vec![
                ("j/k", "Move"),
                ("Enter", "Apply"),
                ("h/l", "Range"),
                ("Esc", "Close"),
            ],
            Some(Modal::Range(_)) => vec![("j/k", "Move"), ("Enter", "Apply"), ("Esc", "Close")],
            Some(Modal::Text(modal)) if modal.kind == TextKind::GoToSubreddit => {
                vec![("Enter", "Open"), ("Esc", "Cancel"), ("Ctrl+U", "Clear")]
            }
            Some(Modal::Text(modal)) if modal.kind == TextKind::AddFilter => {
                vec![("Enter", "Add"), ("Esc", "Back"), ("Ctrl+U", "Clear")]
            }
            Some(Modal::Text(_)) => vec![
                ("Enter", "Submit"),
                ("Tab", "Scope"),
                ("Esc", "Cancel"),
                ("Ctrl+U", "Clear"),
            ],
            Some(Modal::Confirm(_)) => vec![("Enter/y", "Remove"), ("Esc/n", "Cancel")],
            Some(Modal::Settings(_)) => {
                vec![("j/k", "Move"), ("Enter", "Select"), ("Esc", "Close")]
            }
            Some(Modal::ConfigPicker(_)) => {
                vec![("j/k", "Move"), ("Enter", "Install"), ("Esc", "Back")]
            }
            Some(Modal::ThemePicker(_)) => {
                vec![("j/k", "Preview"), ("Enter", "Apply"), ("Esc", "Cancel")]
            }
            Some(Modal::Bookmarks(_)) => vec![
                ("j/k", "Move"),
                ("Enter", "Open"),
                ("d", "Remove"),
                ("Esc", "Close"),
            ],
            Some(Modal::Source(_)) => vec![
                ("j/k", "Move"),
                ("Enter", "Run"),
                ("r", "Test"),
                ("Esc", "Back"),
            ],
            Some(Modal::Filters(_)) => vec![
                ("j/k", "Move"),
                ("a", "Add"),
                ("d", "Remove"),
                ("Esc", "Back"),
            ],
            Some(Modal::CommandPalette(_)) => vec![
                ("type", "Filter"),
                ("↑/↓", "Move"),
                ("Enter", "Run"),
                ("Esc", "Close"),
            ],
            None => Vec::new(),
        }
    } else {
        match view {
            Some(View::Feed(_)) if focus == Focus::Sidebar => vec![
                ("j/k", "Navigate"),
                ("Enter", "Open"),
                ("Tab", "Feed"),
                ("f", "Favorite"),
                ("h", "Hide"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            Some(View::Feed(feed)) => {
                let mut pairs = vec![
                    ("j/k", "Navigate"),
                    ("Enter", "Open"),
                    ("s", "Sort"),
                    ("t", "Top"),
                    ("g", "Go"),
                    ("/", "Search"),
                    ("r", "Refresh"),
                    ("b", "Save"),
                    ("B", "Saved"),
                    ("y", "Copy"),
                    ("Y", "Comments"),
                    ("G", "Sub"),
                ];
                if feed.has_more || feed.loading_more {
                    pairs.push(("↓", "More"));
                }
                pairs.push(("o", "Open"));
                pairs.push(("v", "Image"));
                pairs.push(("?", "Help"));
                pairs.push(("q", "Quit"));
                pairs
            }
            Some(View::Post(_)) if focus == Focus::Comments => vec![
                ("j/k", "Comments"),
                ("Enter", "Toggle"),
                ("c", "Collapse"),
                ("z", "All"),
                ("n", "Unread"),
                ("[ ]", "Root"),
                ("p", "Parent"),
                ("O", "OP"),
                ("s", "Sort"),
                ("Tab", "Post"),
                ("b", "Save"),
                ("y", "Copy"),
                ("Y", "Open"),
                ("G", "Sub"),
                ("Esc", "Back"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            Some(View::Post(_)) => vec![
                ("j/k", "Scroll"),
                ("Tab", "Comments"),
                ("v", "Image"),
                ("o", "Browser"),
                ("x", "Reveal"),
                ("Esc", "Back"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            Some(View::Gallery(_)) => vec![
                ("←/→", "Image"),
                ("o", "Open"),
                ("y", "URL"),
                ("Y", "Comments"),
                ("G", "Sub"),
                ("Esc", "Close"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            None => vec![("q", "Quit")],
        }
    };

    // Pick the richest hint set that actually fits. Previously a fixed
    // 70-column cutoff left an 80-column terminal with a truncated line that
    // hid `q`, `?`, `o` and `v`.
    if line_width(&pairs) > area.width as usize {
        let medium = medium_pairs(view, focus, modal);
        pairs = if line_width(&medium) <= area.width as usize {
            medium
        } else {
            let compact = compact_pairs(view, focus, modal);
            if line_width(&compact) <= area.width as usize {
                compact
            } else {
                micro_pairs(view, focus, modal)
            }
        };
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (index, (key, description)) in pairs.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme.border)));
        }
        spans.push(Span::styled(
            key.to_string(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            description.to_string(),
            Style::default().fg(theme.text_dim),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().fg(theme.text_dim).bg(theme.surface_alt)),
        area,
    );
}

fn line_width(pairs: &[(&str, &str)]) -> usize {
    pairs
        .iter()
        .enumerate()
        .map(|(index, (key, description))| {
            let separator = if index > 0 { 3 } else { 0 };
            separator + width(key) + 1 + width(description)
        })
        .sum()
}

fn medium_pairs(
    view: Option<&View>,
    focus: Focus,
    modal: Option<&Modal>,
) -> Vec<(&'static str, &'static str)> {
    if modal.is_some() {
        return compact_pairs(view, focus, modal);
    }
    match view {
        Some(View::Feed(_)) if focus == Focus::Sidebar => vec![
            ("j/k", "Move"),
            ("Enter", "Open"),
            ("Tab", "Feed"),
            ("f", "Fav"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        Some(View::Feed(_)) => vec![
            ("j/k", "Navigate"),
            ("Enter", "Open"),
            ("s", "Sort"),
            ("g", "Go"),
            ("/", "Search"),
            ("v", "Image"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        Some(View::Post(_)) if focus == Focus::Comments => vec![
            ("j/k", "Comments"),
            ("Enter", "Toggle"),
            ("c", "Collapse"),
            ("s", "Sort"),
            ("Tab", "Post"),
            ("Esc", "Back"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        Some(View::Post(_)) => vec![
            ("j/k", "Scroll"),
            ("Tab", "Comments"),
            ("v", "Image"),
            ("Esc", "Back"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        Some(View::Gallery(_)) => vec![
            ("←/→", "Image"),
            ("o", "Open"),
            ("Esc", "Close"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        None => vec![("q", "Quit")],
    }
}

fn micro_pairs(
    view: Option<&View>,
    focus: Focus,
    modal: Option<&Modal>,
) -> Vec<(&'static str, &'static str)> {
    if let Some(Modal::Confirm(_)) = modal {
        return vec![("y", "Remove"), ("Esc", "Cancel")];
    }
    if let Some(Modal::ThemePicker(_)) = modal {
        return vec![("j/k", "Preview"), ("Esc", "Cancel")];
    }
    if let Some(Modal::Bookmarks(_)) = modal {
        return vec![("↵", "Open"), ("Esc", "Close")];
    }
    if let Some(Modal::Source(_)) = modal {
        return vec![("r", "Test"), ("Esc", "Back")];
    }
    if let Some(Modal::Filters(_)) = modal {
        return vec![("a", "Add"), ("Esc", "Back")];
    }
    if let Some(Modal::CommandPalette(_)) = modal {
        return vec![("↵", "Run"), ("Esc", "Close")];
    }
    if let Some(Modal::Text(modal)) = modal
        && modal.kind == TextKind::GoToSubreddit
    {
        return vec![("Enter", "Open"), ("Esc", "Cancel")];
    }
    if modal.is_some() {
        return vec![("Esc", ""), ("↵", "")];
    }
    match view {
        Some(View::Feed(_)) => vec![("j/k", ""), ("↵", ""), ("s", ""), ("?", ""), ("q", "Quit")],
        Some(View::Post(_)) if focus == Focus::Comments => vec![
            ("j/k", ""),
            ("↵", ""),
            ("c", ""),
            ("Esc", ""),
            ("q", "Quit"),
        ],
        Some(View::Post(_)) => vec![("j/k", ""), ("Tab", ""), ("Esc", ""), ("q", "Quit")],
        Some(View::Gallery(_)) => {
            vec![("←/→", ""), ("o", ""), ("Esc", ""), ("q", "Quit")]
        }
        None => vec![("q", "")],
    }
}

fn compact_pairs(
    view: Option<&View>,
    focus: Focus,
    modal: Option<&Modal>,
) -> Vec<(&'static str, &'static str)> {
    match modal {
        Some(Modal::Help(_)) => vec![("j/k", "Scroll"), ("Esc/?", "Close")],
        Some(Modal::Confirm(_)) => vec![("y", "Remove"), ("Esc", "Cancel")],
        Some(Modal::Text(modal)) if modal.kind == TextKind::GoToSubreddit => {
            vec![("Enter", "Open"), ("Esc", "Close")]
        }
        Some(_) => vec![("Enter", "Apply"), ("Esc", "Close")],
        None => match view {
            Some(View::Feed(_)) => vec![
                ("j/k", "Move"),
                ("Enter", "Open"),
                ("s", "Sort"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            Some(View::Post(_)) if focus == Focus::Comments => vec![
                ("j/k", "Move"),
                ("Enter", "Toggle"),
                ("Esc", "Back"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            Some(View::Post(_)) => vec![
                ("j/k", "Scroll"),
                ("Tab", "Comments"),
                ("Esc", "Back"),
                ("q", "Quit"),
            ],
            Some(View::Gallery(_)) => vec![
                ("←/→", "Image"),
                ("o", "Open"),
                ("Esc", "Close"),
                ("q", "Quit"),
            ],
            None => vec![("q", "Quit")],
        },
    }
}
