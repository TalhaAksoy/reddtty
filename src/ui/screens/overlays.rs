use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};

use crate::app::health::{SourceHealth, format_age, format_duration};
use crate::app::help::{HelpModal, columns_for_width, modal_height};
use crate::app::hit::{HitMap, HitTarget};
use crate::app::modal::{
    AppearanceModal, BookmarksModal, CommandPaletteModal, ConfigPickerModal, ConfirmModal,
    FiltersModal, ImageProtocolModal, InfoModal, Modal, SearchScopeChoice, SettingsModal,
    SortChoice, SortContext, SourceModal, TextKind, TextModal, ThemePickerModal,
};
use crate::app::{NoticeKind, Notification};
use crate::config::presets;
use crate::models::TopRange;
use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, width, wrap_text};

#[allow(clippy::too_many_arguments)]
pub fn render_modal(
    frame: &mut Frame,
    area: Rect,
    modal: &Modal,
    library: &crate::library::Library,
    settings: &crate::config::Settings,
    health: &SourceHealth,
    provider_label: &str,
    demo: bool,
    images: &crate::media::renderer::ImageManager,
    config_corrupt: bool,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    hit_map.push(area, HitTarget::ModalClose);
    match modal {
        Modal::Help(help) => render_help(frame, area, help, theme, hit_map),
        Modal::Sort(sort) => render_sort(frame, area, sort, theme, hit_map),
        Modal::Range(range) => render_range(frame, area, range.selected, theme, hit_map),
        Modal::Text(text) => render_text(frame, area, text, theme, hit_map),
        Modal::Confirm(confirm) => render_confirm(frame, area, confirm, theme, hit_map),
        Modal::Settings(modal) => render_settings(
            frame,
            area,
            modal,
            settings,
            images,
            config_corrupt,
            theme,
            hit_map,
        ),
        Modal::ConfigPicker(picker) => {
            render_config_picker(frame, area, picker, provider_label, theme, hit_map)
        }
        Modal::ThemePicker(picker) => render_theme_picker(frame, area, picker, theme, hit_map),
        Modal::Bookmarks(modal) => render_bookmarks(frame, area, modal, library, theme, hit_map),
        Modal::Source(modal) => render_source(
            frame,
            area,
            modal,
            library,
            health,
            provider_label,
            demo,
            theme,
            hit_map,
        ),
        Modal::Filters(modal) => render_filters(frame, area, modal, library, theme, hit_map),
        Modal::Info(modal) => render_info(frame, area, modal, theme, hit_map),
        Modal::ImagePicker(modal) => {
            render_image_picker(frame, area, modal, settings, images, theme, hit_map)
        }
        Modal::Appearance(modal) => render_appearance(frame, area, modal, settings, theme, hit_map),
        Modal::CommandPalette(modal) => render_command_palette(frame, area, modal, theme, hit_map),
        Modal::Recovery(modal) => render_recovery(frame, area, modal, theme, hit_map),
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

fn render_help(
    frame: &mut Frame,
    area: Rect,
    help: &HelpModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(120);
    let height = modal_height(area.height);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Keyboard Shortcuts", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let columns = columns_for_width(inner.width);
    let total = help.total_rows(columns);
    let visible = inner.height.saturating_sub(1) as usize;
    let offset = help.clamped(area.width, area.height);
    let end = (offset + visible).min(total);
    let col_width = (inner.width / columns.max(1) as u16).max(1);
    let sections = help.visible_sections();

    let mut row = 0usize;
    for view in &sections {
        if row >= offset && row < end {
            let y = inner.y + (row - offset) as u16;
            frame.render_widget(
                Paragraph::new(view.section.title).style(
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.elevated)
                        .add_modifier(Modifier::BOLD),
                ),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
        row += 1;

        for chunk in view.entries.chunks(columns.max(1)) {
            if row >= offset && row < end {
                let y = inner.y + (row - offset) as u16;
                for (column, entry) in chunk.iter().enumerate() {
                    let x = inner.x + column as u16 * col_width;
                    let line = shortcut_line(
                        entry.keys,
                        entry.desc,
                        theme,
                        col_width.saturating_sub(1) as usize,
                        theme.elevated,
                    );
                    frame.render_widget(
                        Paragraph::new(line).style(Style::default().bg(theme.elevated)),
                        Rect::new(x, y, col_width, 1),
                    );
                }
            }
            row += 1;
        }
        row += 1;
    }

    if sections.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching shortcuts.")
                .style(Style::default().fg(theme.muted).bg(theme.elevated))
                .alignment(Alignment::Center),
            Rect::new(inner.x, inner.y + inner.height / 2, inner.width, 1),
        );
    }

    let hidden = total.saturating_sub(visible);
    let footer_y = inner.bottom().saturating_sub(1);
    let left = if help.searching {
        format!("Search: {}▏", help.query)
    } else if !help.query.is_empty() {
        format!("Search: {}  ·  / edit · Esc clear", help.query)
    } else {
        "j/k scroll · PgUp/PgDn page · / search".to_string()
    };
    if inner.width >= 54 {
        frame.render_widget(
            Paragraph::new(ellipsize(&left, (inner.width / 2) as usize))
                .style(Style::default().fg(theme.muted).bg(theme.elevated))
                .alignment(Alignment::Left),
            Rect::new(inner.x, footer_y, inner.width / 2, 1),
        );
        let right = if help.searching {
            "Esc clear · ↑/↓ scroll".to_string()
        } else {
            format!("{end}/{total} · {hidden} more · Esc close")
        };
        frame.render_widget(
            Paragraph::new(right)
                .style(Style::default().fg(theme.muted).bg(theme.elevated))
                .alignment(Alignment::Right),
            Rect::new(inner.x + inner.width / 2, footer_y, inner.width / 2, 1),
        );
    } else {
        let right = if help.searching {
            "Esc clear".to_string()
        } else {
            format!("{hidden} more · Esc close")
        };
        frame.render_widget(
            Paragraph::new(right)
                .style(Style::default().fg(theme.muted).bg(theme.elevated))
                .alignment(Alignment::Right),
            Rect::new(inner.x, footer_y, inner.width, 1),
        );
    }
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

fn render_recovery(
    frame: &mut Frame,
    area: Rect,
    modal: &crate::app::modal::RecoveryModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(72);
    let height = (crate::app::modal::RecoveryModal::ACTIONS.len() as u16 * 2 + 9)
        .min(area.height.saturating_sub(2))
        .max(10);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Configuration recovery", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    let mut lines = vec![
        Line::from(Span::styled(
            "Your existing configuration could not be loaded.",
            Style::default()
                .fg(theme.danger)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "The original file was preserved. Saves are blocked until you choose an action.",
            Style::default().fg(theme.text_dim),
        )),
        Line::from(""),
    ];
    for (index, action) in crate::app::modal::RecoveryModal::ACTIONS.iter().enumerate() {
        let selected = index == modal.selected;
        let marker = if selected { " ▸ " } else { "   " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{action}"),
            if selected {
                Style::default()
                    .fg(theme.selected_fg)
                    .bg(theme.selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            },
        )));
        hit_map.push(
            Rect::new(inner.x, inner.y + 3 + index as u16, inner.width, 1),
            HitTarget::ModalItem(index),
        );
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "j/k move · Enter select · Esc close",
        Style::default().fg(theme.muted),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_settings(
    frame: &mut Frame,
    area: Rect,
    modal: &SettingsModal,
    settings: &crate::config::Settings,
    images: &crate::media::renderer::ImageManager,
    config_corrupt: bool,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let height = if config_corrupt { 19 } else { 17 };
    let rect = layout::centered(area, area.width.min(54), height);
    modal_frame(frame, rect, "Settings", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    let mut rows = vec![
        (
            "Change config".to_string(),
            "switch source now (no restart)".to_string(),
        ),
        (
            format!("Theme: {}", modal.theme.label()),
            "preview light, dark, Catppuccin, contrast".to_string(),
        ),
        (
            format!(
                "Images: {}",
                if images.enabled() {
                    images.protocol_label()
                } else {
                    "disabled"
                }
            ),
            format!(
                "configured {} · detected {} · effective {}",
                settings.images.protocol.label(),
                images.auto_protocol_label(),
                images.protocol_label()
            ),
        ),
        (
            "Appearance".to_string(),
            format!(
                "{} time · {} motion",
                if settings.ui.relative_time {
                    "relative"
                } else {
                    "absolute"
                },
                if settings.ui.reduce_motion {
                    "reduced"
                } else {
                    "full"
                }
            ),
        ),
        (
            "Source health".to_string(),
            "host, latency, errors, retry".to_string(),
        ),
        (
            "Filters".to_string(),
            "hide read, keywords, subs, authors, domains".to_string(),
        ),
        (
            "Clear read history".to_string(),
            "confirm before marking everything unread".to_string(),
        ),
    ];
    if config_corrupt {
        rows.push((
            "Configuration recovery".to_string(),
            "original preserved; saves blocked".to_string(),
        ));
    }
    for (index, (title, description)) in rows.iter().enumerate() {
        let y = inner.y.saturating_add(index as u16 * 2);
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let selected = index == modal.selected;
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
    provider_label: &str,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(76);
    let height = (presets::PRESETS.len() as u16 * 3 + 5)
        .min(area.height.saturating_sub(2))
        .max(8);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Choose content source", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    let mut available = inner.height.saturating_sub(1);
    if picker.active.is_none() {
        frame.render_widget(
            Paragraph::new(ellipsize(
                &format!("Current source: {provider_label}"),
                inner.width as usize,
            ))
            .style(
                Style::default()
                    .fg(theme.info)
                    .bg(theme.elevated)
                    .add_modifier(Modifier::BOLD),
            ),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        available = available.saturating_sub(1);
    }
    let list_y = inner.y + u16::from(picker.active.is_none());
    let row_height = if available >= 12 { 3 } else { 2 };
    let visible = (available / row_height).max(1) as usize;
    let total = presets::PRESETS.len() + 1;
    let offset = picker.selected.saturating_sub(visible.saturating_sub(1));
    for (slot, index) in (offset..total).take(visible).enumerate() {
        let (name, description, is_custom) = match presets::PRESETS.get(index) {
            Some(preset) => (preset.name, preset.description, false),
            None => (
                "+ Custom Redlib instance…",
                "Self-hosted or private instance; enter its HTTPS root URL.",
                true,
            ),
        };
        let y = list_y.saturating_add((slot as u16) * row_height);
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
        let active = if picker.active == Some(index) {
            "  ● active"
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(format!("{marker}{name}{active}")).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        if row_height >= 3 {
            frame.render_widget(
                Paragraph::new(format!("    {description}"))
                    .style(Style::default().fg(theme.muted).bg(bg)),
                Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
            );
        }
        hit_map.push(
            Rect::new(inner.x, y, inner.width, row_height),
            HitTarget::ModalItem(index),
        );
        let _ = is_custom;
    }
    let footer = format!(
        "{}–{} / {} · j/k · Enter select · Esc back · network stays opt-in",
        offset + 1,
        (offset + visible).min(total),
        total
    );
    frame.render_widget(
        Paragraph::new(ellipsize(&footer, inner.width as usize))
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_theme_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &ThemePickerModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(56);
    let height = (crate::config::ThemeMode::ALL.len() as u16 * 2 + 5)
        .min(area.height.saturating_sub(2))
        .max(9);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Theme", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    for (index, mode) in crate::config::ThemeMode::ALL.iter().enumerate() {
        let y = inner.y.saturating_add(index as u16 * 2);
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
            Paragraph::new(format!("{marker}{}", mode.label())).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(if selected {
                "    Enter apply · Esc cancel"
            } else {
                ""
            })
            .style(Style::default().fg(theme.muted).bg(theme.elevated)),
            Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 2),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new("j/k preview · Enter apply · Esc cancel")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_bookmarks(
    frame: &mut Frame,
    area: Rect,
    modal: &BookmarksModal,
    library: &crate::library::Library,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(78);
    let height = area.height.saturating_sub(2).clamp(8, 26);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Bookmarks", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let filtered = modal.filtered(library);
    let footer = if modal.searching {
        format!("Search: {}▏  ·  Enter/Esc finish", modal.query)
    } else if modal.query.is_empty() {
        "j/k move · PgUp/PgDn · Enter open · / search · d remove · Esc close".to_string()
    } else {
        format!(
            "Filter “{}” · j/k move · / edit · d remove · Esc close",
            modal.query
        )
    };

    if filtered.is_empty() {
        let (heading, detail) = if library.bookmarks.is_empty() {
            (
                "No bookmarks yet.",
                "Press b on a post to save an offline snapshot.",
            )
        } else {
            (
                "No bookmarks match this search.",
                "Press / to edit the search or Esc to clear it.",
            )
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    heading,
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(detail, Style::default().fg(theme.text_dim))),
            ])
            .style(Style::default().bg(theme.elevated)),
            inner,
        );
        frame.render_widget(
            Paragraph::new(ellipsize(&footer, inner.width as usize))
                .style(Style::default().fg(theme.muted).bg(theme.elevated))
                .alignment(Alignment::Right),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
        return;
    }

    let rows = inner.height.saturating_sub(1) as usize / 2;
    let selected_pos = modal.selected.min(filtered.len().saturating_sub(1));
    let offset = selected_pos.saturating_sub(rows.saturating_sub(1));
    for (slot, position) in (offset..filtered.len()).take(rows).enumerate() {
        let index = filtered[position];
        let saved = &library.bookmarks[index];
        let selected = position == selected_pos;
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
        let mut flags = String::new();
        if !library.is_post_read(&saved.post.id) {
            flags.push_str("  ● unread");
        }
        let title = ellipsize(
            &crate::utils::text::sanitize(&saved.post.title),
            inner.width.saturating_sub(4) as usize,
        );
        let y = inner.y + (slot as u16) * 2;
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
        let comments = if saved.post.num_comments == 0 {
            "no comments".to_string()
        } else {
            format!("{} comments · not stored", saved.post.num_comments)
        };
        let meta = format!(
            "    r/{} · u/{} · saved {} · {comments}{flags}",
            crate::utils::text::sanitize(&saved.post.subreddit),
            crate::utils::text::sanitize(&saved.post.author),
            crate::utils::time::relative_now(saved.saved_at),
        );
        frame.render_widget(
            Paragraph::new(ellipsize(&meta, inner.width as usize))
                .style(Style::default().fg(theme.muted).bg(theme.elevated)),
            Rect::new(inner.x, y.saturating_add(1), inner.width, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 2),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new(ellipsize(&footer, inner.width as usize))
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_image_picker(
    frame: &mut Frame,
    area: Rect,
    modal: &ImageProtocolModal,
    settings: &crate::config::Settings,
    images: &crate::media::renderer::ImageManager,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(64);
    let height = (crate::config::ImageProtocolSetting::ALL.len() as u16 * 2 + 5)
        .min(area.height.saturating_sub(2))
        .max(10);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Image protocol", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    for (index, setting) in crate::config::ImageProtocolSetting::ALL.iter().enumerate() {
        let y = inner.y.saturating_add(index as u16 * 2);
        if y >= inner.bottom().saturating_sub(1) {
            break;
        }
        let selected = index == modal.selected;
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
        let description = match setting {
            crate::config::ImageProtocolSetting::Auto => {
                if settings.images.enabled {
                    "detected at startup"
                } else {
                    "detected; images currently disabled"
                }
            }
            crate::config::ImageProtocolSetting::Kitty => "pixel graphics (Kitty/Ghostty)",
            crate::config::ImageProtocolSetting::Iterm2 => "iTerm2 inline images",
            crate::config::ImageProtocolSetting::Sixel => "sixel graphics",
            crate::config::ImageProtocolSetting::Halfblocks => "reliable Unicode blocks",
            crate::config::ImageProtocolSetting::Disabled => "turn image rendering off",
        };
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", setting.label())).style(
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
        Paragraph::new(format!(
            "effective {} · detected {} · j/k · Enter apply · Esc back",
            images.protocol_label(),
            images.auto_protocol_label()
        ))
        .style(Style::default().fg(theme.muted).bg(theme.elevated))
        .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_appearance(
    frame: &mut Frame,
    area: Rect,
    modal: &AppearanceModal,
    settings: &crate::config::Settings,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let rect = layout::centered(area, area.width.min(54), 9);
    modal_frame(frame, rect, "Appearance", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);
    for (index, (title, description)) in [
        (
            format!(
                "Timestamps: {}",
                if settings.ui.relative_time {
                    "relative"
                } else {
                    "absolute"
                }
            ),
            "relative times such as “2h ago” or fixed dates".to_string(),
        ),
        (
            format!(
                "Motion: {}",
                if settings.ui.reduce_motion {
                    "reduced"
                } else {
                    "full"
                }
            ),
            "stop spinner animation when reduced".to_string(),
        ),
    ]
    .iter()
    .enumerate()
    {
        let y = inner.y.saturating_add(index as u16 * 2);
        let selected = index == modal.selected;
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
        Paragraph::new("j/k move · Enter toggle · Esc back")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_command_palette(
    frame: &mut Frame,
    area: Rect,
    modal: &CommandPaletteModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(68);
    let height = area.height.saturating_sub(2).clamp(10, 24);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Commands", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let query = format!("> {}▏", modal.query);
    frame.render_widget(
        Paragraph::new(ellipsize(&query, inner.width as usize))
            .style(Style::default().fg(theme.accent).bg(theme.elevated)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    if let Some(error) = &modal.error {
        frame.render_widget(
            Paragraph::new(ellipsize(error, inner.width as usize))
                .style(Style::default().fg(theme.danger).bg(theme.elevated)),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
    let commands = modal.filtered();
    let list_y = if modal.error.is_some() { 3 } else { 2 };
    let rows = inner.height.saturating_sub(list_y as u16 + 1) as usize;
    let offset = if modal.selected >= rows {
        modal.selected + 1 - rows
    } else {
        0
    };
    if commands.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching command.")
                .style(Style::default().fg(theme.muted).bg(theme.elevated)),
            Rect::new(inner.x, inner.y + list_y as u16, inner.width, 1),
        );
    }
    for (slot, index) in (offset..commands.len()).take(rows).enumerate() {
        let selected = index == modal.selected;
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
        let marker = if selected { " ▸ " } else { "   " };
        let y = inner.y + list_y as u16 + slot as u16;
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", commands[index].label())).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
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
        Paragraph::new("type or run commands like sort new · ↑/↓ move · Enter · Esc")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_info(
    frame: &mut Frame,
    area: Rect,
    modal: &InfoModal,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let info = &modal.info;
    let width = area.width.saturating_sub(4).min(78);
    let height = area.height.saturating_sub(2).clamp(10, 30);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Info", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let heading = |text: &'static str| {
        Line::from(Span::styled(
            text,
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let mut lines = vec![
        heading("ABOUT"),
        Line::from(Span::styled(
            crate::utils::text::sanitize(&info.title),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if let Some(subtitle) = &info.subtitle {
        lines.push(Line::from(Span::styled(
            crate::utils::text::sanitize(subtitle),
            Style::default().fg(theme.text_dim),
        )));
    }
    if let Some((members, online)) = info.members {
        lines.push(Line::from(Span::styled(
            format!(
                "{} members  ·  {} online",
                crate::utils::text::format_count(members as i64),
                crate::utils::text::format_count(online as i64)
            ),
            Style::default().fg(theme.text_dim),
        )));
    }
    if info.loading {
        lines.push(Line::from(Span::styled(
            "loading community…",
            Style::default().fg(theme.muted),
        )));
    }
    if let Some(error) = &info.error {
        lines.push(Line::from(Span::styled(
            crate::utils::text::ellipsize(
                &crate::utils::text::sanitize(error),
                inner.width.saturating_sub(2) as usize,
            ),
            Style::default().fg(theme.danger),
        )));
    }
    if let Some(description) = &info.description {
        lines.push(Line::from(""));
        for wrapped in crate::utils::text::wrap_text(
            &crate::utils::text::sanitize(description),
            inner.width.saturating_sub(2) as usize,
        )
        .into_iter()
        .take(8)
        {
            lines.push(Line::from(Span::styled(
                wrapped,
                Style::default().fg(theme.text_dim),
            )));
        }
    }
    if !info.post_lines.is_empty() {
        lines.push(Line::from(""));
        lines.push(heading("DETAILS"));
        for line in &info.post_lines {
            lines.push(Line::from(Span::styled(
                crate::utils::text::sanitize(line),
                Style::default().fg(theme.text_dim),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(heading("SUGGESTED COMMUNITIES"));
    for name in crate::app::sidebar::SUGGESTED.iter().take(6) {
        lines.push(Line::from(vec![
            Span::styled("▸ ", Style::default().fg(theme.accent)),
            Span::styled(format!("r/{name}"), Style::default().fg(theme.text_dim)),
        ]));
    }
    if !info.actions.is_empty() {
        lines.push(Line::from(""));
        lines.push(heading("ACTIONS"));
        for (key, description) in &info.actions {
            lines.push(Line::from(vec![
                Span::styled(format!("{key:<5}"), Style::default().fg(theme.accent)),
                Span::styled(
                    crate::utils::text::sanitize(description),
                    Style::default().fg(theme.text_dim),
                ),
            ]));
        }
    }
    let visible = inner.height.saturating_sub(1) as usize;
    let offset = modal.scroll.min(lines.len().saturating_sub(visible));
    let lines: Vec<Line> = lines.into_iter().skip(offset).take(visible).collect();
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(1),
        ),
    );
    frame.render_widget(
        Paragraph::new("j/k scroll · actions run immediately · Esc or i to close")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn render_filters(
    frame: &mut Frame,
    area: Rect,
    modal: &FiltersModal,
    library: &crate::library::Library,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(66);
    let height = area.height.saturating_sub(2).clamp(10, 20);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Local filters", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let mut rows: Vec<(String, String)> = vec![(
        format!(
            "[{}] Hide read posts",
            if library.filters.hide_read { "x" } else { " " }
        ),
        "posts you marked read are skipped locally".into(),
    )];
    for entry in library.filters.entries() {
        rows.push((
            format!("{}  {}", entry.kind.label(), entry.value),
            String::new(),
        ));
    }
    if rows.len() == 1 {
        rows.push((
            "No filter rules yet.".into(),
            "press a to add keyword, sub, author or domain".into(),
        ));
    }

    let visible = inner.height.saturating_sub(1) as usize;
    let offset = if modal.selected >= visible {
        modal.selected + 1 - visible
    } else {
        0
    };
    for (slot, index) in (offset..rows.len()).take(visible).enumerate() {
        let (label, hint) = &rows[index];
        let selected = index == modal.selected;
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
        let y = inner.y + slot as u16;
        let text = if hint.is_empty() {
            format!("{marker}{label}")
        } else {
            format!("{marker}{label}  ·  {hint}")
        };
        frame.render_widget(
            Paragraph::new(ellipsize(&text, inner.width as usize)).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
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
        Paragraph::new("a add · d remove · C clear · Enter toggle · Esc back")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

#[allow(clippy::too_many_arguments)]
fn render_source(
    frame: &mut Frame,
    area: Rect,
    modal: &SourceModal,
    library: &crate::library::Library,
    health: &SourceHealth,
    provider_label: &str,
    demo: bool,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let width = area.width.saturating_sub(4).min(74);
    let height = area.height.saturating_sub(2).clamp(14, 22);
    let rect = layout::centered(area, width, height);
    modal_frame(frame, rect, "Source health", theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let status = health
        .last_status
        .map(|status| status.label().to_string())
        .unwrap_or_else(|| "No request yet this session".into());
    let latency = match (health.last_latency, health.average_latency()) {
        (Some(last), Some(avg)) => {
            format!(
                "{} last · {} average",
                format_duration(last),
                format_duration(avg)
            )
        }
        _ => "no samples yet".into(),
    };
    let rate = health
        .success_rate()
        .map(|rate| format!("{rate}% of {} requests", health.requests))
        .unwrap_or_else(|| "no requests yet".into());
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Source   ", Style::default().fg(theme.muted)),
            Span::styled(
                crate::utils::text::sanitize(provider_label),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mode     ", Style::default().fg(theme.muted)),
            Span::styled(
                if demo {
                    "Demo · offline synthetic data"
                } else {
                    "Live · read-only provider"
                },
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("Status   ", Style::default().fg(theme.muted)),
            Span::styled(
                status,
                Style::default()
                    .fg(if health.is_degraded() {
                        theme.danger
                    } else {
                        theme.success
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Last ok  ", Style::default().fg(theme.muted)),
            Span::styled(
                format_age(health.last_success_age()),
                Style::default().fg(theme.text_dim),
            ),
            Span::styled("   Attempt  ", Style::default().fg(theme.muted)),
            Span::styled(
                format_age(health.last_attempt_age()),
                Style::default().fg(theme.text_dim),
            ),
        ]),
        Line::from(vec![
            Span::styled("Latency  ", Style::default().fg(theme.muted)),
            Span::styled(latency, Style::default().fg(theme.text_dim)),
        ]),
        Line::from(vec![
            Span::styled("Reliability ", Style::default().fg(theme.muted)),
            Span::styled(rate, Style::default().fg(theme.text_dim)),
        ]),
    ];
    if let Some(error) = &health.last_error {
        lines.push(Line::from(Span::styled("", Style::default())));
        lines.push(Line::from(Span::styled(
            "Last error",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )));
        for line in wrap_text(
            &crate::utils::text::sanitize(error),
            inner.width.saturating_sub(1) as usize,
        )
        .into_iter()
        .take(4)
        {
            lines.push(Line::from(Span::styled(
                line,
                Style::default().fg(theme.danger),
            )));
        }
    }
    lines.push(Line::from(Span::styled("", Style::default())));
    lines.push(Line::from(vec![
        Span::styled("Library  ", Style::default().fg(theme.muted)),
        Span::styled(
            format!(
                "{} bookmarks · {} local filter rules",
                library.bookmarks.len(),
                library.filters.rule_count()
            ),
            Style::default().fg(theme.text_dim),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Cache    ", Style::default().fg(theme.muted)),
        Span::styled(
            "provider responses are in-memory for this session",
            Style::default().fg(theme.text_dim),
        ),
    ]));

    let action_space = SourceModal::ACTIONS.len() as u16 + 2;
    let available = inner.height.saturating_sub(action_space) as usize;
    lines.truncate(available);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(action_space),
        ),
    );

    let action_y = inner
        .bottom()
        .saturating_sub(SourceModal::ACTIONS.len() as u16 + 1);
    for (index, action) in SourceModal::ACTIONS.iter().enumerate() {
        let selected = index == modal.selected;
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
        let y = action_y + index as u16;
        frame.render_widget(
            Paragraph::new(format!("{marker}{action}")).style(
                Style::default().fg(fg).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, y, inner.width / 2, 1),
        );
        hit_map.push(
            Rect::new(inner.x, y, inner.width, 1),
            HitTarget::ModalItem(index),
        );
    }
    frame.render_widget(
        Paragraph::new("j/k move · Enter run · r test · c change · Esc back")
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

    let message_lines = message.len();
    let mut lines: Vec<Line> = message
        .into_iter()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.text))))
        .collect();
    lines.push(Line::from(""));
    let verb = confirm.action.verb();
    lines.push(Line::from(vec![
        Span::styled(
            " Enter/y ",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.danger)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {verb}   "), Style::default().fg(theme.text_dim)),
        Span::styled(
            " Esc/n ",
            Style::default()
                .fg(theme.badge_fg)
                .bg(theme.badge_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(theme.text)),
    ]));
    let action_y = inner.y.saturating_add(message_lines as u16 + 1);
    let yes_width = 9.min(inner.width);
    hit_map.push(
        Rect::new(inner.x, action_y, yes_width, 1),
        HitTarget::ConfirmYes,
    );
    let no_x = inner.x.saturating_add(yes_width + verb.len() as u16 + 2);
    hit_map.push(
        Rect::new(
            no_x,
            action_y,
            8.min(inner.width.saturating_sub(yes_width)),
            1,
        ),
        HitTarget::ConfirmNo,
    );
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
    let is_filter = text.kind == TextKind::AddFilter;
    let height = if is_search {
        11
    } else if is_filter {
        10
    } else {
        9
    };
    let rect = layout::centered(area, area.width.min(64), height);
    modal_frame(frame, rect, &text.title, theme);
    let inner = inner_of(rect);
    hit_map.push(inner, HitTarget::ModalBody);

    let label = match text.kind {
        TextKind::Search => "Search: ",
        TextKind::GoToSubreddit => "Subreddit: ",
        TextKind::AddFilter => "Filter: ",
        TextKind::CustomRedlib => "Instance URL: ",
    };
    let value = &text.value;
    let show_prefix = !is_search
        && !is_filter
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
                TextKind::AddFilter => {
                    "keyword:ai · sub:politics · author:name · domain:example.com".to_string()
                }
                TextKind::CustomRedlib => {
                    "HTTPS root URL · no credentials · you will confirm network access next"
                        .to_string()
                }
            };
            frame.render_widget(
                Paragraph::new(ellipsize(&help, inner.width as usize))
                    .style(Style::default().fg(theme.muted).bg(theme.elevated)),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
    }

    let submit_label = if is_search {
        " Search "
    } else if is_filter {
        " Add "
    } else {
        " Open "
    };
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
    } else if is_filter {
        "Enter add · Esc back"
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
    if notifications.is_empty() || area.width < 12 || area.height == 0 {
        return;
    }
    let count = notifications.len().min(3).min(area.height as usize);
    let width = area.width.saturating_sub(2).min(62);
    let start_y = area.bottom().saturating_sub(count as u16);
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
