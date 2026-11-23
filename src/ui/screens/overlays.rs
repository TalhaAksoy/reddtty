use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};

use crate::app::hit::{HitMap, HitTarget};
use crate::app::modal::{
    ConfigPickerModal, ConfirmModal, Modal, SearchScopeChoice, SettingsModal, SortChoice,
    SortContext, TextKind, TextModal,
};
use crate::app::{NoticeKind, Notification};
use crate::config::presets;
use crate::models::TopRange;
use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, width, wrap_text};

pub fn render_modal(
    frame: &mut Frame,
    area: Rect,
    modal: &Modal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    hit_map.push(area, HitTarget::ModalClose);
    match modal {
        Modal::Help => render_help(frame, area, theme, hit_map),
        Modal::Sort(sort) => render_sort(frame, area, sort, theme, hit_map),
        Modal::Range(range) => render_range(frame, area, range.selected, theme, hit_map),
        Modal::Text(text) => render_text(frame, area, text, theme, hit_map),
        Modal::Confirm(confirm) => render_confirm(frame, area, confirm, theme, hit_map),
        Modal::Settings(settings) => render_settings(frame, area, settings, theme, hit_map),
        Modal::ConfigPicker(picker) => render_config_picker(frame, area, picker, theme, hit_map),
    }
}

fn modal_frame(frame: &mut Frame, rect: Rect, title: &str, theme: &Theme) {
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(Line::from(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.elevated))
        .padding(Padding::horizontal(1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.elevated)),
        inner,
    );
    // inner is used for content; caller re-derives it.
    let _ = inner;
}

fn inner_of(rect: Rect) -> Rect {
    Rect::new(
        rect.x + 2,
        rect.y + 1,
        rect.width.saturating_sub(4),
        rect.height.saturating_sub(2),
    )
}

fn render_help(frame: &mut Frame, area: Rect, theme: &Theme, hit_map: &mut HitMap) {
    let shortcuts: [(&str, &str); 23] = [
        ("j / k", "Navigate"),
        ("↑ / ↓", "Same"),
        ("Enter", "Open / toggle"),
        ("Esc", "Back / close"),
        ("Tab", "Switch focus"),
        ("s", "Sort"),
        ("t", "Top range"),
        ("g", "Go to sub"),
        ("/", "Search"),
        ("r", "Refresh"),
        ("o", "Browser"),
        ("v", "Image viewer"),
        ("x", "Reveal NSFW"),
        ("c", "Collapse"),
        ("f", "Favorite"),
        ("h", "Sidebar"),
        ("T", "Theme"),
        ("PgUp / PgDn", "Page"),
        ("Home / End", "Jump"),
        ("Mouse", "Click / scroll"),
        ("?", "Help"),
        ("S", "Settings"),
        ("q", "Quit"),
    ];
    let two_columns = area.width >= 56;
    let per_column = if two_columns {
        shortcuts.len().div_ceil(2)
    } else {
        shortcuts.len()
    };
    let height = (per_column as u16 + 4)
        .min(area.height.saturating_sub(2))
        .max(6);
    let width = if two_columns {
        area.width.min(78)
    } else {
        area.width.min(52)
    };
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Keyboard Shortcuts", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let rows = inner.height.saturating_sub(2) as usize;
    if two_columns {
        let col_width = inner.width / 2;
        for (index, (key, desc)) in shortcuts.iter().enumerate().take(per_column * 2) {
            let col = index / per_column;
            let row = index % per_column;
            if row >= rows {
                continue;
            }
            let x = inner.x + col as u16 * col_width;
            let line = shortcut_line(
                key,
                desc,
                theme,
                (col_width.saturating_sub(1)) as usize,
                theme.elevated,
            );
            frame.render_widget(
                Paragraph::new(line).style(Style::default().bg(theme.elevated)),
                Rect::new(x, inner.y + row as u16, col_width, 1),
            );
        }
    } else {
        for (row, (key, desc)) in shortcuts.iter().take(rows).enumerate() {
            let line = shortcut_line(key, desc, theme, inner.width as usize, theme.elevated);
            frame.render_widget(
                Paragraph::new(line).style(Style::default().bg(theme.elevated)),
                Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
            );
        }
    }
    let shown = if two_columns {
        (rows * 2).min(shortcuts.len())
    } else {
        rows.min(shortcuts.len())
    };
    let hidden = shortcuts.len().saturating_sub(shown);
    let footer = if hidden == 0 {
        "Esc or ? to close".to_string()
    } else {
        format!("{hidden} more · Esc close")
    };
    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn shortcut_line(
    key: &str,
    desc: &str,
    theme: &Theme,
    width: usize,
    bg: ratatui::style::Color,
) -> Line<'static> {
    let key = ellipsize(key, 12);
    let desc = ellipsize(desc, width.saturating_sub(12));
    Line::from(vec![
        Span::styled(
            format!("{key:<12}"),
            Style::default()
                .fg(theme.warning)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(theme.text_dim).bg(bg)),
    ])
}

fn render_settings(
    frame: &mut Frame,
    area: Rect,
    settings: &SettingsModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let rect = layout::centered(area, area.width.min(48), 9);
    modal_frame(frame, rect, "Settings", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    let rows = [
        (
            "Change config".to_string(),
            "replace the active config file".to_string(),
        ),
        (
            format!("Theme: {}", settings.theme.label()),
            "toggle dark / light".to_string(),
        ),
    ];
    for (index, (title, description)) in rows.iter().enumerate() {
        let y = inner.y.saturating_add(index as u16 * 2);
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let selected = index == settings.selected;
        let bg = if selected {
            theme.selected_bg
        } else {
            theme.elevated
        };
        let fg = if selected {
            theme.selected_fg
        } else {
            theme.text
        };
        let marker = if selected { " ▸ " } else { "   " };
        frame.render_widget(
            Paragraph::new(format!("{marker}{title}")).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(format!("    {description}"))
                .style(Style::default().fg(theme.muted).bg(theme.elevated)),
            Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 2),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new("j/k move · Enter select · Esc close")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_config_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &ConfigPickerModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(76);
    let height = (presets::PRESETS.len() as u16 * 3 + 5)
        .min(area.height.saturating_sub(2))
        .max(10);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Choose config", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    for (index, preset) in presets::PRESETS.iter().enumerate() {
        let y = inner.y.saturating_add(index as u16 * 3);
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let selected = index == picker.selected;
        let bg = if selected {
            theme.selected_bg
        } else {
            theme.elevated
        };
        let fg = if selected {
            theme.selected_fg
        } else {
            theme.text
        };
        let marker = if selected { " ▸ " } else { "   " };
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", preset.name)).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(format!("    {}", preset.description))
                .style(Style::default().fg(theme.muted).bg(theme.elevated)),
            Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 3),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new("j/k move · Enter install · Esc back")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    confirm: &ConfirmModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.min(58);
    let inner_width = width.saturating_sub(4).max(12);
    let message = wrap_text(
        &crate::utils::text::sanitize(&confirm.message),
        inner_width as usize,
    );
    let height = (message.len() as u16 + 5)
        .min(area.height.saturating_sub(2))
        .max(7);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, &confirm.title, theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let mut lines: Vec<Line> = message
        .into_iter()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.text))))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " Enter/y ",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.danger)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" remove   ", Style::default().fg(theme.text_dim)),
        Span::styled(
            " Esc/n ",
            Style::default()
                .fg(theme.badge_fg)
                .bg(theme.badge_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(theme.text)),
    ]));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_sort(
    frame: &mut Frame,
    area: Rect,
    sort: &crate::app::modal::SortModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let height = (sort.len() as u16 + 3)
        .min(area.height.saturating_sub(2))
        .max(6);
    let rect = layout::centered(area, area.width.min(48), height);
    modal_frame(frame, rect, "Sort", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    let rows = inner.height.saturating_sub(1) as usize;
    let offset = if sort.selected >= rows {
        sort.selected + 1 - rows
    } else {
        0
    };
    for (slot, index) in (offset..sort.len()).take(rows).enumerate() {
        let selected = index == sort.selected;
        let bg = if selected {
            theme.selected_bg
        } else {
            theme.elevated
        };
        let fg = if selected {
            theme.selected_fg
        } else {
            theme.text_dim
        };
        let prefix = if selected { " ▸ " } else { "   " };
        let label = sort.label(index);
        let is_range = matches!(sort.choices.get(index), Some(SortChoice::Range));
        let marker = if is_range {
            format!("{prefix}{label}  ‹ ›")
        } else {
            format!("{prefix}{label}")
        };
        let y = inner.y + slot as u16;
        frame.render_widget(
            Paragraph::new(ellipsize(&marker, inner.width as usize)).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        if is_range {
            hit_map.push(
                Rect::new(inner.x, y, inner.width, 1),
                HitTarget::ModalRangeItem(index),
            );
        } else {
            hit_map.push(
                Rect::new(inner.x, y, inner.width, 1),
                HitTarget::ModalItem(index),
            );
        }
    }
    let hint = match sort.context {
        SortContext::Feed => "Enter apply · h/l range · Esc close",
        SortContext::Comments => "Enter apply · Esc close",
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_range(
    frame: &mut Frame,
    area: Rect,
    selected: usize,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let rect = layout::centered(area, area.width.min(42), TopRange::ALL.len() as u16 + 4);
    modal_frame(frame, rect, "Top time range", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    for (index, range) in TopRange::ALL.iter().enumerate() {
        if index as u16 >= inner.height.saturating_sub(1) {
            break;
        }
        let is_selected = index == selected;
        let bg = if is_selected {
            theme.selected_bg
        } else {
            theme.elevated
        };
        let fg = if is_selected {
            theme.selected_fg
        } else {
            theme.text_dim
        };
        let prefix = if is_selected { " ▸ " } else { "   " };
        let y = inner.y + index as u16;
        frame.render_widget(
            Paragraph::new(format!("{prefix}{}", range.label())).style(
                Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 1),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new("Enter apply · Esc close")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_text(
    frame: &mut Frame,
    area: Rect,
    text: &TextModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let is_search = text.kind == TextKind::Search;
    let height = if is_search { 11 } else { 9 };
    let rect = layout::centered(area, area.width.min(64), height);
    modal_frame(frame, rect, &text.title, theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let label = if is_search { "Search: " } else { "Subreddit: " };
    let value = &text.value;
    let show_prefix = !is_search
        && !value.starts_with("r/")
        && !value.starts_with("/r/")
        && !value.starts_with("http");
    let prefix = if show_prefix {
        text.prompt.as_str()
    } else {
        ""
    };
    let line = Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(theme.muted)),
        Span::styled(prefix.to_string(), Style::default().fg(theme.accent)),
        Span::styled(value.clone(), Style::default().fg(theme.text)),
        Span::styled("▏", Style::default().fg(theme.accent)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme.elevated)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    let cursor_x = inner.x + width(label) as u16 + width(prefix) as u16 + width(value) as u16;
    frame.set_cursor_position(ratatui::layout::Position::new(
        cursor_x.min(inner.right().saturating_sub(1)),
        inner.y,
    ));

    let mut y = inner.y + 2;
    if is_search && y < inner.bottom() {
        let scope = format!(
            "Scope: {}   (Tab to change)",
            if text.scope == SearchScopeChoice::Current {
                text.current_subreddit
                    .as_ref()
                    .map(|s| format!("r/{s}"))
                    .unwrap_or_else(|| "All Reddit".into())
            } else {
                "All Reddit".into()
            }
        );
        let scope_rect = Rect::new(inner.x, y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(scope).style(Style::default().fg(theme.info).bg(theme.elevated)),
            scope_rect,
        );
        hit_map.push(scope_rect, HitTarget::ModalScope);
        y = y.saturating_add(2);
    }

    if y < inner.bottom() {
        if let Some(error) = &text.error {
            frame.render_widget(
                Paragraph::new(ellipsize(error, inner.width as usize))
                    .style(Style::default().fg(theme.danger).bg(theme.elevated)),
                Rect::new(inner.x, y, inner.width, 1),
            );
        } else {
            let help = match text.kind {
                TextKind::GoToSubreddit => "Examples: rust  ·  r/rust  ·  r/rust/".to_string(),
                TextKind::Search => {
                    "Searches the current provider. Empty queries are ignored.".to_string()
                }
            };
            frame.render_widget(
                Paragraph::new(ellipsize(&help, inner.width as usize))
                    .style(Style::default().fg(theme.muted).bg(theme.elevated)),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
    }

    let submit_label = if is_search { " Search " } else { " Open " };
    let submit_width = width(submit_label) as u16 + 2;
    let submit_rect = Rect::new(
        inner.right().saturating_sub(submit_width),
        inner.bottom().saturating_sub(1),
        submit_width,
        1,
    );
    frame.render_widget(
        Paragraph::new(submit_label)
            .style(
                Style::default()
                    .fg(theme.on_accent)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        submit_rect,
    );
    hit_map.push(submit_rect, HitTarget::ModalSubmit);
    let hint = if is_search {
        "Enter search · Esc cancel"
    } else {
        "Enter open · Esc cancel"
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Left),
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(1),
            inner.width.saturating_sub(submit_width + 1),
            1,
        ),
    );
}

pub fn render_notifications(
    frame: &mut Frame,
    area: Rect,
    notifications: &VecDeque<Notification>,
    theme: &Theme,
) {
    if notifications.is_empty() || area.width < 12 || area.height < 2 {
        return;
    }
    let count = notifications.len().min(3);
    let width = area.width.saturating_sub(2).min(62);
    let start_y = area.bottom().saturating_sub(1).saturating_sub(count as u16);
    for (i, notification) in notifications.iter().rev().take(count).enumerate() {
        let y = start_y + i as u16;
        if y >= area.bottom() {
            break;
        }
        let (glyph, fg, bg) = match notification.kind {
            NoticeKind::Info => ("ℹ", theme.info, theme.surface_alt),
            NoticeKind::Success => ("✓", theme.success, theme.surface_alt),
            NoticeKind::Warning => ("!", theme.warning, theme.surface_alt),
            NoticeKind::Error => ("✕", theme.danger, theme.surface_alt),
        };
        let text = ellipsize(&notification.text, width.saturating_sub(4) as usize);
        let line = Line::from(vec![
            Span::styled(
                format!(" {glyph} "),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(text, Style::default().fg(theme.text).bg(bg)),
        ]);
        frame.render_widget(
            Paragraph::new(line)
                .style(Style::default().bg(bg))
                .alignment(Alignment::Left),
            Rect::new(area.right().saturating_sub(width), y, width, 1),
        );
    }
}
