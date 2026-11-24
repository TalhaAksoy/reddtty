use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::Focus;
use crate::app::hit::{HitMap, HitTarget, Pane};
use crate::app::post::PostView;
use crate::config::Settings;
use crate::media::renderer::{ImageManager, ImageStatus};
use crate::models::Post;
use crate::ui::layout::{self, DetailParams, DetailRects};
use crate::ui::theme::Theme;
use crate::ui::widgets::comment_tree;
use crate::utils::text::{ellipsize, format_count, sanitize, strip_markdown, wrap_text};
use crate::utils::time::{absolute, relative_now};

pub fn layout_for(post: &Post, area: Rect, settings: &Settings, hidden: bool) -> DetailRects {
    let content_width = area.width.saturating_sub(2).max(1);
    let title_lines = wrap_text(&sanitize(&post.title), content_width as usize)
        .len()
        .max(1) as u16;
    let has_body = !post.body.trim().is_empty();
    let body_lines = if has_body {
        wrap_text(
            &strip_markdown(&sanitize(&post.body)),
            content_width.max(8) as usize,
        )
        .len()
        .min(200) as u16
    } else {
        0
    };
    layout::detail(
        area,
        DetailParams {
            title_lines,
            body_lines,
            has_comments: true,
            has_image: !hidden
                && settings.images.enabled
                && settings.images.protocol != crate::config::ImageProtocolSetting::Disabled
                && post.preview_image().is_some(),
            has_body: has_body && !hidden,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    post_view: &PostView,
    settings: &Settings,
    focus: Focus,
    images: &mut ImageManager,
    spinner: &str,
    spinner_index: usize,
    revealed: &HashSet<String>,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let hidden = {
        let show = revealed.contains(&post_view.post.id);
        post_view.post.is_hidden(
            settings.content.show_nsfw || show,
            !settings.content.hide_spoilers || show,
        )
    };
    let panels = layout::post(area, settings);
    let rects = layout_for(&post_view.post, panels.main, settings, hidden);
    let post = &post_view.post;

    // Post pane background.
    frame.render_widget(Paragraph::new("").style(theme.base()), rects.post_pane);
    hit_map.push(rects.post_pane, HitTarget::Pane(Pane::PostBody));

    render_meta(frame, rects.meta, post, settings, theme);
    render_title(frame, rects.title, post, theme);
    render_metrics(frame, rects.metrics, post, theme);

    if let Some(image_rect) = rects.image {
        render_post_image(
            frame,
            image_rect,
            post,
            images,
            spinner_index,
            theme,
            hit_map,
        );
    }

    if focus == Focus::PostBody {
        let rail = Rect::new(
            rects.post_pane.x,
            rects.meta.y,
            1,
            rects.post_pane.bottom().saturating_sub(rects.meta.y),
        );
        crate::ui::widgets::draw_focus_rail(frame, rail, true, theme, theme.bg);
    }

    render_body(frame, rects.body, post_view, hidden, focus, theme);

    // Comments header and list.
    frame.render_widget(Paragraph::new("").style(theme.base()), rects.comments_pane);
    if rects.comments_pane.height > 0 {
        let header = rects.comments_header;
        if header.height > 0 {
            let count = format_count(post.num_comments as i64);
            let mut spans = vec![
                Span::styled(
                    "COMMENTS",
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {count}"), Style::default().fg(theme.muted)),
            ];
            if let Some(error) = &post_view.post_error {
                spans.push(Span::styled(
                    format!(
                        "   ·   {}",
                        ellipsize(&crate::utils::text::sanitize(error), 60)
                    ),
                    Style::default().fg(theme.danger),
                ));
            } else if post_view.comments_loading && !post_view.rows.is_empty() {
                spans.push(Span::styled(
                    format!("   ·   {spinner} loading"),
                    Style::default().fg(theme.muted),
                ));
            }
            frame.render_widget(
                Paragraph::new(Line::from(spans)).style(theme.base()),
                Rect::new(header.x, header.y, header.width, 1),
            );

            let sort_label = format!("Sort: {} ▾", post_view.comments_sort.label());
            let sort_width = crate::utils::text::width(&sort_label) as u16;
            if header.width > sort_width + 10 {
                let sort_rect = Rect::new(
                    header.right().saturating_sub(sort_width),
                    header.y,
                    sort_width,
                    1,
                );
                frame.render_widget(
                    Paragraph::new(sort_label)
                        .style(Style::default().fg(theme.info).bg(theme.bg))
                        .alignment(ratatui::layout::Alignment::Right),
                    sort_rect,
                );
                hit_map.push(sort_rect, HitTarget::CommentSort);
            }
            if header.height >= 2 {
                crate::ui::widgets::draw_rule(
                    frame,
                    Rect::new(header.x, header.y + 1, header.width, 1),
                    theme,
                );
            }
        }
        if post.is_deleted() {
            frame.render_widget(
                Paragraph::new("Comments are unavailable for removed posts.")
                    .style(Style::default().fg(theme.muted).bg(theme.bg))
                    .alignment(ratatui::layout::Alignment::Center),
                rects.comments_list,
            );
        } else {
            comment_tree::render(
                frame,
                rects.comments_list,
                post_view,
                settings,
                theme,
                focus,
                hidden,
                spinner,
                hit_map,
            );
        }
    }

    if let Some(right) = panels.right {
        render_community_panel(frame, right, post_view, spinner, theme);
    }
}

fn render_community_panel(
    frame: &mut Frame,
    area: Rect,
    post_view: &PostView,
    spinner: &str,
    theme: &Theme,
) {
    if area.height < 4 || area.width < 12 {
        return;
    }
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.surface));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let heading = |text: &'static str| {
        Line::from(Span::styled(
            text,
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ))
    };
    lines.push(heading("ABOUT"));
    lines.push(Line::from(Span::styled(
        format!(
            "r/{}",
            crate::utils::text::sanitize(&post_view.post.subreddit)
        ),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(subreddit) = &post_view.subreddit {
        if !subreddit.title.is_empty() && !subreddit.title.eq_ignore_ascii_case(&subreddit.name) {
            lines.push(Line::from(Span::styled(
                crate::utils::text::sanitize(&subreddit.title),
                Style::default().fg(theme.text_dim),
            )));
        }
        lines.push(Line::from(Span::styled(
            format!(
                "{} members  ·  {} online",
                crate::utils::text::format_count(subreddit.subscribers as i64),
                crate::utils::text::format_count(subreddit.active_users as i64)
            ),
            Style::default().fg(theme.text_dim),
        )));
        if !subreddit.description.is_empty() {
            lines.push(Line::from(""));
            for wrapped in wrap_text(
                &crate::utils::text::sanitize(&subreddit.description),
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
    } else if let Some(error) = &post_view.subreddit_error {
        lines.push(Line::from(Span::styled(
            ellipsize(
                &crate::utils::text::sanitize(error),
                inner.width.saturating_sub(2) as usize,
            ),
            Style::default().fg(theme.danger),
        )));
    } else if post_view.subreddit_request.is_some() {
        lines.push(Line::from(Span::styled(
            format!("{spinner} loading community…"),
            Style::default().fg(theme.muted),
        )));
    }

    lines.push(Line::from(""));
    lines.push(heading("POST"));
    lines.push(Line::from(Span::styled(
        format!(
            "u/{}  ·  {} points  ·  {} comments",
            crate::utils::text::sanitize(&post_view.post.author),
            crate::utils::text::format_count(post_view.post.score),
            crate::utils::text::format_count(post_view.post.num_comments as i64)
        ),
        Style::default().fg(theme.text_dim),
    )));
    lines.push(Line::from(Span::styled(
        post_view.post.content.kind_label().to_string(),
        Style::default().fg(theme.info),
    )));

    lines.push(Line::from(""));
    lines.push(heading("SUGGESTED COMMUNITIES"));
    for name in crate::app::sidebar::SUGGESTED.iter().take(6) {
        lines.push(Line::from(vec![
            Span::styled("▸ ", Style::default().fg(theme.accent)),
            Span::styled(format!("r/{name}"), Style::default().fg(theme.text_dim)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(heading("ACTIONS"));
    for (key, desc) in [
        ("G", "open subreddit"),
        ("Y", "open comments"),
        ("b", "bookmark"),
        ("Esc", "back to feed"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("{key:<5}"), Style::default().fg(theme.accent)),
            Span::styled(desc.to_string(), Style::default().fg(theme.text_dim)),
        ]));
    }

    lines.truncate(inner.height as usize);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.surface).fg(theme.text))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_meta(frame: &mut Frame, area: Rect, post: &Post, settings: &Settings, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let time = if settings.ui.relative_time {
        relative_now(post.created_utc)
    } else {
        absolute(post.created_utc)
    };
    let mut spans = vec![
        Span::styled(
            format!("r/{}", sanitize(&post.subreddit)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   •   ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("u/{}", sanitize(&post.author)),
            Style::default().fg(theme.text_dim),
        ),
        Span::styled("   •   ", Style::default().fg(theme.muted)),
        Span::styled(time, Style::default().fg(theme.muted)),
    ];
    if post.stickied {
        spans.push(Span::styled(
            "   PINNED",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if post.nsfw {
        spans.push(Span::styled(
            "   NSFW",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.danger)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if post.spoiler {
        spans.push(Span::styled(
            "   SPOILER",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(flair) = &post.flair
        && !flair.is_empty()
    {
        spans.push(Span::styled(
            format!("   {}", sanitize(flair)),
            Style::default().fg(theme.info),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

fn render_title(frame: &mut Frame, area: Rect, post: &Post, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let lines: Vec<Line> = wrap_text(&sanitize(&post.title), area.width as usize)
        .into_iter()
        .take(area.height as usize)
        .map(|text| {
            Line::from(Span::styled(
                text,
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(theme.base()), area);
}

fn render_metrics(frame: &mut Frame, area: Rect, post: &Post, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let kind = post.content.kind_label();
    let mut spans = vec![
        Span::styled(
            format!("▲ {} points", format_count(post.score)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("    ◍ {} comments", format_count(post.num_comments as i64)),
            Style::default().fg(theme.text_dim),
        ),
        Span::styled(
            format!("    [{kind}]"),
            Style::default().fg(theme.badge_fg).bg(theme.badge_bg),
        ),
    ];
    if post.has_gallery() {
        spans.push(Span::styled(
            format!("    {} images", post.images().len()),
            Style::default().fg(theme.info),
        ));
        spans.push(Span::styled(
            "   ·   v to browse",
            Style::default().fg(theme.muted),
        ));
    } else if post.preview_image().is_some() {
        spans.push(Span::styled(
            "   ·   v to view image",
            Style::default().fg(theme.muted),
        ));
    }
    if let Some(url) = post.open_url() {
        spans.push(Span::styled(
            format!("   ·   o to open {}", ellipsize(&display_domain(&url), 28)),
            Style::default().fg(theme.muted),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

#[allow(clippy::too_many_arguments)]
fn render_post_image(
    frame: &mut Frame,
    area: Rect,
    post: &Post,
    images: &mut ImageManager,
    spinner_index: usize,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize))
            .style(Style::default().bg(theme.surface_alt))
            .wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
    hit_map.push(area, HitTarget::OpenImage);
    let Some(url) = post.preview_image() else {
        return;
    };
    if !images.enabled() {
        frame.render_widget(
            Paragraph::new("images disabled")
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(theme.muted).bg(theme.surface_alt)),
            area,
        );
        return;
    }
    match images.poll(url, area.width, area.height) {
        ImageStatus::Ready(protocol) => {
            let size = protocol.size();
            let w = size.width.min(area.width);
            let h = size.height.min(area.height);
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            frame.render_widget(
                ratatui_image::Image::new(protocol).allow_clipping(true),
                Rect::new(x, y, w, h),
            );
            if post.has_gallery() {
                let label = format!(" 1 / {}  ·  v to browse ", post.images().len());
                frame.render_widget(
                    Paragraph::new(label)
                        .style(Style::default().fg(theme.on_accent).bg(theme.accent))
                        .alignment(ratatui::layout::Alignment::Right),
                    Rect::new(
                        area.right().saturating_sub(28),
                        area.bottom().saturating_sub(1),
                        28.min(area.width),
                        1,
                    ),
                );
            }
        }
        ImageStatus::Loading | ImageStatus::NotRequested => {
            frame.render_widget(
                Paragraph::new(format!(
                    "{}  loading image…   ·   v to open viewer",
                    crate::ui::spinner_char(spinner_index)
                ))
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(theme.muted).bg(theme.surface_alt)),
                area,
            );
        }
        ImageStatus::Failed(error) => {
            frame.render_widget(
                Paragraph::new(format!(
                    "Image unavailable: {}   ·   o to open externally",
                    ellipsize(
                        &crate::utils::text::sanitize(error),
                        area.width.saturating_sub(25) as usize
                    )
                ))
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(theme.danger).bg(theme.surface_alt)),
                area,
            );
        }
        ImageStatus::Disabled => {
            frame.render_widget(
                Paragraph::new("images unavailable")
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(Style::default().fg(theme.muted).bg(theme.surface_alt)),
                area,
            );
        }
    }
}

fn render_body(
    frame: &mut Frame,
    area: Rect,
    post_view: &PostView,
    hidden: bool,
    focus: Focus,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if hidden {
        frame.render_widget(
            Paragraph::new("Content hidden. Press x to reveal this post.")
                .style(Style::default().fg(theme.warning).bg(theme.bg)),
            area,
        );
        return;
    }
    if post_view.post.is_deleted() {
        frame.render_widget(
            Paragraph::new("This post was removed, so its content is no longer available.")
                .style(Style::default().fg(theme.muted).bg(theme.bg)),
            area,
        );
        return;
    }
    if post_view.body_len() == 0 {
        frame.render_widget(
            Paragraph::new("No text content.").style(Style::default().fg(theme.muted).bg(theme.bg)),
            area,
        );
        return;
    }
    let offset = post_view.body_offset as usize;
    let lines: Vec<Line> = post_view
        .body_lines()
        .iter()
        .skip(offset)
        .take(area.height as usize)
        .cloned()
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)).style(theme.base()), area);
    if post_view.body_len() > area.height as usize {
        let indicator = format!(
            "↕ {}/{}",
            (offset + area.height as usize).min(post_view.body_len()),
            post_view.body_len()
        );
        frame.render_widget(
            Paragraph::new(ellipsize(&indicator, area.width as usize))
                .style(Style::default().fg(theme.muted).bg(theme.bg))
                .alignment(ratatui::layout::Alignment::Right),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );
    }
    let _ = focus;
}

fn display_domain(url: &str) -> String {
    let raw = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string());
    crate::utils::text::sanitize(&raw)
}
