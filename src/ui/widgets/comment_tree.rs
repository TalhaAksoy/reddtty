use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use crate::app::Focus;
use crate::app::hit::{HitMap, HitTarget, Pane};
use crate::app::post::PostView;
use crate::config::Settings;
use crate::ui::theme::Theme;
use crate::ui::widgets::draw_scrollbar;
use crate::utils::text::{ellipsize, format_count};
use crate::utils::time::relative_now;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    post: &PostView,
    settings: &Settings,
    theme: &Theme,
    focus: Focus,
    hidden: bool,
    spinner: &str,
    hit_map: &mut HitMap,
) {
    if area.height == 0 || area.width < 6 {
        return;
    }
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.bg)),
        area,
    );
    hit_map.push(area, HitTarget::Pane(Pane::Comments));

    if hidden {
        frame.render_widget(
            Paragraph::new("Comments hidden until the post is revealed (press x).")
                .style(Style::default().fg(theme.warning).bg(theme.bg))
                .alignment(ratatui::layout::Alignment::Center),
            area,
        );
        return;
    }

    if post.rows.is_empty() {
        let (text, style) = if post.comments_loading {
            (
                format!("{spinner}  Loading comments…"),
                Style::default().fg(theme.muted),
            )
        } else if let Some(error) = &post.comments_error {
            (
                format!(
                    "Could not load comments.\n{}",
                    ellipsize(&crate::utils::text::sanitize(error), 120)
                ),
                Style::default().fg(theme.danger),
            )
        } else {
            (
                "No comments yet.\nBe the first to reply on Reddit.".to_string(),
                Style::default().fg(theme.muted),
            )
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(style.bg(theme.bg))
                .alignment(ratatui::layout::Alignment::Center),
            area,
        );
        if post.comments_error.is_some() {
            let retry = Rect::new(
                area.x + area.width.saturating_sub(14) / 2,
                area.y + area.height / 2 + 2,
                14.min(area.width),
                1,
            );
            if retry.y < area.bottom() {
                frame.render_widget(
                    Paragraph::new("[ Retry ]")
                        .style(
                            Style::default()
                                .fg(theme.on_accent)
                                .bg(theme.accent)
                                .add_modifier(Modifier::BOLD),
                        )
                        .alignment(ratatui::layout::Alignment::Center),
                    retry,
                );
                hit_map.push(retry, HitTarget::Retry);
            }
        }
        return;
    }

    let content_x = area.x + 1;
    let content_w = area.width.saturating_sub(2).max(1);
    let range = post.visible_comments(area.height);
    let relative = settings.ui.relative_time;

    for index in range {
        let Some(row) = post.rows.get(index) else {
            continue;
        };
        let selected = index == post.selected;
        let selected_bg = if selected {
            if focus == Focus::Comments {
                theme.selected_bg
            } else {
                theme.surface_alt
            }
        } else {
            theme.bg
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut header_spans: Vec<Span<'static>> = Vec::new();
        header_spans.push(Span::styled(
            row.prefix.clone(),
            Style::default().fg(if selected {
                theme.border_active
            } else {
                theme.muted
            }),
        ));

        if row.is_more {
            let label = if post.more_loading.contains(&row.id) {
                format!("{spinner}  loading more replies…")
            } else if row.more_count > 0 {
                format!("▸ [ load {} more replies ]", row.more_count)
            } else {
                "▸ [ load more replies ]".to_string()
            };
            header_spans.push(Span::styled(
                label,
                Style::default()
                    .fg(theme.info)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            if row.collapsed {
                header_spans.push(Span::styled("▶ ", Style::default().fg(theme.warning)));
            }
            if row.distinguished {
                header_spans.push(Span::styled(
                    "[MOD] ",
                    Style::default()
                        .fg(theme.on_accent)
                        .bg(theme.info)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            let author_style = if row.deleted || row.removed {
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default()
                    .fg(if selected {
                        theme.selected_fg
                    } else {
                        theme.accent
                    })
                    .add_modifier(Modifier::BOLD)
            };
            header_spans.push(Span::styled(format!("u/{}", row.author), author_style));
            if !row.deleted && !row.removed && row.author == post.post.author {
                header_spans.push(Span::styled(
                    " OP",
                    Style::default().fg(theme.on_accent).bg(theme.accent),
                ));
            }
            if !row.deleted && !row.removed {
                header_spans.push(Span::styled(
                    format!("   ▲ {}", format_count(row.score)),
                    Style::default().fg(theme.text_dim),
                ));
            }
            if row.created_utc > 0 {
                let time = if relative {
                    relative_now(row.created_utc)
                } else {
                    crate::utils::time::absolute(row.created_utc)
                };
                header_spans.push(Span::styled(
                    format!("   {time}"),
                    Style::default().fg(theme.muted),
                ));
            }
            if row.collapsed && row.hidden_replies > 0 {
                header_spans.push(Span::styled(
                    format!("   ({} replies hidden)", row.hidden_replies),
                    Style::default().fg(theme.warning),
                ));
            }
        }
        lines.push(Line::from(header_spans).style(Style::default().bg(selected_bg)));

        if !row.is_more {
            for body in &row.body_lines {
                let style = if row.deleted || row.removed {
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::ITALIC)
                } else {
                    Style::default().fg(theme.text_dim)
                };
                lines.push(
                    Line::from(vec![
                        Span::styled(
                            row.body_prefix.clone(),
                            Style::default().fg(theme.border).bg(selected_bg),
                        ),
                        Span::styled(body.clone(), style.bg(selected_bg)),
                    ])
                    .style(Style::default().bg(selected_bg)),
                );
            }
        }
        // One gap line after the comment.
        lines.push(Line::from("").style(Style::default().bg(theme.bg)));

        let row_top = post.prefix_height(index) as i64;
        let rel = row_top - post.offset as i64;
        let cut = if rel < 0 { (-rel) as usize } else { 0 };
        let start_y = if rel < 0 {
            area.y
        } else {
            area.y.saturating_add(rel as u16)
        };
        if start_y >= area.bottom() {
            continue;
        }
        if cut >= lines.len() {
            continue;
        }
        lines.drain(0..cut);
        let available = area.bottom().saturating_sub(start_y) as usize;
        if available == 0 {
            continue;
        }
        lines.truncate(available);
        let visible_height = lines.len() as u16;
        frame.render_widget(
            Paragraph::new(Text::from(lines)).style(Style::default().bg(theme.bg)),
            Rect::new(content_x, start_y, content_w, visible_height),
        );

        // Selection marker in the gutter column.
        if selected {
            let buf = frame.buffer_mut();
            for line in 0..visible_height {
                if let Some(cell) =
                    buf.cell_mut(ratatui::layout::Position::new(area.x, start_y + line))
                {
                    cell.set_symbol("▌")
                        .set_fg(theme.accent)
                        .set_bg(selected_bg);
                }
            }
        }

        let hit_top = start_y.max(area.y);
        let hit_height =
            (row.height as usize).min(area.bottom().saturating_sub(hit_top) as usize) as u16;
        if hit_height > 0 {
            hit_map.push(
                Rect::new(area.x, hit_top, area.width.saturating_sub(1), hit_height),
                HitTarget::CommentRow(index),
            );
        }
    }

    let (position, total, viewport) = post.comment_scrollbar(area.height);
    if area.width > 2 {
        draw_scrollbar(
            frame,
            Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
            position,
            total,
            viewport,
            theme,
            theme.bg,
        );
    }

    // Show an inline error banner even when stale rows are visible.
    if let Some(error) = &post.comments_error
        && area.height > 2
    {
        let banner = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        let error = crate::utils::text::sanitize(error);
        frame.render_widget(
            Paragraph::new(ellipsize(
                &format!("Comments: {error}"),
                area.width as usize,
            ))
            .style(Style::default().fg(theme.danger).bg(theme.surface_alt)),
            banner,
        );
    }
}
