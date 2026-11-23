use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::Focus;
use crate::app::hit::{HitMap, HitTarget, Pane};
use crate::app::sidebar::{self, SidebarEntry, SidebarState};
use crate::config::Settings;
use crate::models::FeedSpec;
use crate::providers::ProviderCapabilities;
use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, width};

struct LineItem {
    entry: Option<SidebarEntry>,
    text: String,
    section: bool,
    flat_index: Option<usize>,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    settings: &Settings,
    caps: ProviderCapabilities,
    state: &SidebarState,
    active_spec: Option<&FeedSpec>,
    focus: Focus,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    if area.height == 0 || area.width < 4 {
        return;
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(if focus == Focus::Sidebar {
            theme.border_active
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.surface).fg(theme.text));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    hit_map.push(area, HitTarget::Pane(Pane::Sidebar));

    let sections = sidebar::sections(settings, caps);
    let mut items = Vec::new();
    let mut flat = 0usize;
    let mut selected_line = 0usize;
    for section in sections {
        items.push(LineItem {
            entry: None,
            text: section.title.to_string(),
            section: true,
            flat_index: None,
        });
        if section.entries.is_empty() && !section.empty_hint.is_empty() {
            items.push(LineItem {
                entry: None,
                text: section.empty_hint.to_string(),
                section: false,
                flat_index: None,
            });
        }
        for entry in section.entries {
            if flat == state.selected {
                selected_line = items.len();
            }
            items.push(LineItem {
                entry: Some(entry),
                text: String::new(),
                section: false,
                flat_index: Some(flat),
            });
            flat += 1;
        }
        items.push(LineItem {
            entry: None,
            text: String::new(),
            section: false,
            flat_index: None,
        });
    }

    let visible_height = inner.height as usize;
    let offset = if selected_line >= visible_height {
        selected_line + 1 - visible_height
    } else {
        0
    };

    let max_width = inner.width.saturating_sub(2);
    let mut y = inner.y;
    for (index, item) in items.iter().enumerate().skip(offset) {
        if y >= inner.bottom() {
            break;
        }
        if item.section {
            let text = ellipsize(&item.text, max_width as usize);
            frame.render_widget(
                Paragraph::new(text).style(
                    Style::default()
                        .fg(theme.muted)
                        .bg(theme.surface)
                        .add_modifier(Modifier::BOLD),
                ),
                Rect::new(inner.x + 1, y, max_width, 1),
            );
            y = y.saturating_add(1);
            continue;
        }
        let Some(entry) = &item.entry else {
            if !item.text.is_empty() {
                frame.render_widget(
                    Paragraph::new(ellipsize(&item.text, max_width.saturating_sub(2) as usize))
                        .style(Style::default().fg(theme.muted).bg(theme.surface)),
                    Rect::new(inner.x + 2, y, max_width.saturating_sub(1), 1),
                );
            }
            y = y.saturating_add(1);
            continue;
        };
        let selected = focus == Focus::Sidebar && item.flat_index == Some(state.selected);
        let active = active_spec.is_some_and(|spec| spec == &entry.spec);
        let background = if selected {
            theme.selected_bg
        } else {
            theme.surface
        };
        let marker = if active { "┃" } else { " " };
        let marker_style = if active {
            Style::default().fg(theme.accent).bg(background)
        } else {
            Style::default().fg(theme.muted).bg(background)
        };
        let label = if entry.favorite {
            format!("★ {}", entry.label)
        } else {
            entry.label.clone()
        };
        let label = ellipsize(&label, max_width.saturating_sub(1) as usize);
        let fg = if selected {
            theme.selected_fg
        } else if active {
            theme.accent
        } else {
            theme.text_dim
        };
        let line = Line::from(vec![
            Span::styled(marker.to_string(), marker_style),
            Span::styled(
                format!(" {label}"),
                Style::default()
                    .fg(fg)
                    .bg(background)
                    .add_modifier(if active || selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]);
        let _ = index;
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(background)),
            Rect::new(inner.x, y, inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 1),
            HitTarget::Sidebar(entry.spec.clone()),
        );
        y = y.saturating_add(1);
    }

    // Footer hint if there is room.
    if inner.height > 3 && y + 1 < inner.bottom() {
        let hint = if focus == Focus::Sidebar {
            "h hide · f favorite"
        } else {
            "h hide · Tab focus"
        };
        frame.render_widget(
            Paragraph::new(ellipsize(hint, inner.width as usize))
                .style(Style::default().fg(theme.muted).bg(theme.surface)),
            Rect::new(
                inner.x + 1,
                inner.bottom().saturating_sub(1),
                inner.width.saturating_sub(1),
                1,
            ),
        );
    }
    let _ = width;
}
