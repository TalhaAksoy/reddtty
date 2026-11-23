//! Post card: the primary visual unit of the feed.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};

use crate::config::Settings;
use crate::media::renderer::{ImageManager, ImageStatus};
use crate::models::{Post, PostContent};
use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, format_count, strip_markdown, width, wrap_text};
use crate::utils::time::{absolute, relative_now};

#[derive(Clone, Debug)]
pub struct CardLayout {
    pub inner_width: u16,
    pub meta_y: u16,
    pub title_y: u16,
    pub title_height: u16,
    pub note_y: Option<u16>,
    pub body_y: Option<u16>,
    pub body_height: u16,
    pub image_y: Option<u16>,
    pub image_height: u16,
    pub footer_y: u16,
    pub total_height: u16,
}

pub fn compact_for_viewport(viewport: u16) -> bool {
    // Below roughly 24 visible rows, the regular card spacing only fits one
    // post. Tight spacing keeps the feed browsable without dropping metadata.
    viewport < 24
}

fn preview_height(inner_width: u16, compact: bool) -> u16 {
    if inner_width < 12 {
        return 0;
    }
    if compact {
        if inner_width >= 40 { 3 } else { 2 }
    } else if inner_width >= 84 {
        8
    } else if inner_width >= 60 {
        6
    } else if inner_width >= 40 {
        4
    } else {
        3
    }
}

pub fn card_layout(
    post: &Post,
    card_width: u16,
    compact: bool,
    preview_enabled: bool,
    hidden: bool,
) -> CardLayout {
    let placeholder = |inner_width: u16| CardLayout {
        inner_width,
        meta_y: 1,
        title_y: 1,
        title_height: 1,
        note_y: None,
        body_y: None,
        body_height: 0,
        image_y: None,
        image_height: 0,
        footer_y: 1,
        total_height: 3,
    };
    if card_width < 10 {
        return placeholder(card_width.saturating_sub(4));
    }

    let inner_width = card_width.saturating_sub(4);
    let title_source = crate::utils::text::sanitize(&post.title);
    let title_height = wrap_text(&title_source, inner_width as usize)
        .len()
        .clamp(1, 3) as u16;

    let removed = post.is_deleted();
    let has_body = !removed && !post.body.trim().is_empty();
    let max_body = if compact { 1 } else { 2 };
    let body_height = if has_body {
        let wrapped = wrap_text(
            &strip_markdown(&crate::utils::text::sanitize(&post.body)),
            inner_width as usize,
        )
        .len()
        .max(1) as u16;
        wrapped.min(max_body)
    } else {
        0
    };

    let has_image = !removed && !hidden && preview_enabled && post.preview_image().is_some();
    let image_height = if has_image {
        preview_height(inner_width, compact)
    } else {
        0
    };
    let image_height = if image_height < 2 { 0 } else { image_height };

    let tight = compact;
    let mut y = 1u16;
    let meta_y = y;
    y = y.saturating_add(1);

    // Compact cards keep one blank line between metadata and title, but drop
    // the blank lines around body/footer so two text cards can fit on a
    // 60x20 or 80x24 terminal.
    let title_y = y.saturating_add(1);
    y = y.saturating_add(1 + title_height);

    let mut note_y = None;
    let mut body_y = None;
    let mut image_y = None;

    if hidden || removed {
        y = y.saturating_add(1);
        note_y = Some(y);
        y = y.saturating_add(1);
    } else {
        if body_height > 0 {
            if !tight {
                y = y.saturating_add(1);
            }
            body_y = Some(y);
            y = y.saturating_add(body_height);
        }
        if image_height > 0 {
            y = y.saturating_add(1);
            image_y = Some(y);
            y = y.saturating_add(image_height);
        }
    }

    let footer_y = if tight { y } else { y.saturating_add(1) };
    let total_height = footer_y.saturating_add(2);

    CardLayout {
        inner_width,
        meta_y,
        title_y,
        title_height,
        note_y,
        body_y,
        body_height,
        image_y,
        image_height,
        footer_y,
        total_height,
    }
}

pub fn card_height(
    post: &Post,
    card_width: u16,
    compact: bool,
    preview_enabled: bool,
    hidden: bool,
) -> u16 {
    card_layout(post, card_width, compact, preview_enabled, hidden).total_height
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    post: &Post,
    selected: bool,
    theme: &Theme,
    settings: &Settings,
    images: &mut ImageManager,
    compact: bool,
    revealed: bool,
    spinner: usize,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let hidden = post.is_hidden(
        settings.content.show_nsfw || revealed,
        !settings.content.hide_spoilers || revealed,
    );
    let layout = card_layout(
        post,
        area.width.min(10_000),
        compact,
        settings.images.enabled
            && settings.images.protocol != crate::config::ImageProtocolSetting::Disabled
            && settings.images.preview_in_feed,
        hidden,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border(selected))
        .style(theme.card(selected))
        .padding(Padding::horizontal(1));
    frame.render_widget(block, area);

    let inner = Rect::new(area.x + 2, area.y, layout.inner_width.max(1), area.height);
    // Content rows live strictly between the top and bottom borders. A
    // partially visible card must never paint text over its own bottom border.
    let content_bottom = area.height.saturating_sub(1);
    let fg = if selected {
        theme.selected_fg
    } else {
        theme.text
    };

    // Meta row
    if layout.meta_y < content_bottom {
        let meta = post_meta_line(post, theme, settings);
        frame.render_widget(
            Paragraph::new(meta).style(Style::default().fg(if selected {
                theme.text_dim
            } else {
                theme.muted
            })),
            Rect::new(inner.x, area.y + layout.meta_y, inner.width, 1),
        );
    }

    // Title
    let wrapped_title = wrap_text(
        &crate::utils::text::sanitize(&post.title),
        inner.width as usize,
    );
    let title_total = wrapped_title.len();
    let title_lines: Vec<Line> = wrapped_title
        .into_iter()
        .take(layout.title_height as usize)
        .enumerate()
        .map(|(index, line)| {
            let mut text = line;
            if index + 1 == layout.title_height as usize
                && title_total > layout.title_height as usize
            {
                while width(&text) + 1 > inner.width as usize && !text.is_empty() {
                    text.pop();
                }
                text.push('…');
            }
            if index == 0 && post.stickied {
                text = format!("📌 {text}");
            }
            Line::from(Span::styled(
                text,
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    if layout.title_y < content_bottom {
        let height = layout
            .title_height
            .min(content_bottom.saturating_sub(layout.title_y));
        if height > 0 {
            frame.render_widget(
                Paragraph::new(title_lines).wrap(Wrap { trim: true }),
                Rect::new(inner.x, area.y + layout.title_y, inner.width, height),
            );
        }
    }

    if let Some(note_y) = layout.note_y.filter(|y| *y < content_bottom) {
        let note = if post.nsfw && !settings.content.show_nsfw && !revealed {
            "[NSFW] content hidden — press x to reveal"
        } else if post.spoiler && settings.content.hide_spoilers && !revealed {
            "[SPOILER] content hidden — press x to reveal"
        } else if post.is_deleted() {
            "This post was removed by moderators"
        } else {
            "content hidden"
        };
        frame.render_widget(
            Paragraph::new(note).style(
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::ITALIC),
            ),
            Rect::new(inner.x, area.y + note_y, inner.width, 1),
        );
    }

    if let (Some(body_y), true) = (layout.body_y, layout.body_height > 0)
        && body_y < content_bottom
    {
        let body_height = layout
            .body_height
            .min(content_bottom.saturating_sub(body_y));
        if body_height > 0 {
            let source = strip_markdown(&crate::utils::text::sanitize(&post.body));
            let wrapped = wrap_text(&source, inner.width as usize);
            let total = wrapped.len();
            let lines: Vec<Line> = wrapped
                .into_iter()
                .take(body_height as usize)
                .enumerate()
                .map(|(index, mut line)| {
                    if index + 1 == body_height as usize && total > body_height as usize {
                        while width(&line) + 1 > inner.width as usize && !line.is_empty() {
                            line.pop();
                        }
                        line.push('…');
                    }
                    Line::from(Span::styled(line, Style::default().fg(theme.text_dim)))
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines),
                Rect::new(inner.x, area.y + body_y, inner.width, body_height),
            );
        }
    }

    if let (Some(image_y), true) = (layout.image_y, layout.image_height >= 2)
        && image_y < content_bottom
    {
        let image_height = layout
            .image_height
            .min(content_bottom.saturating_sub(image_y));
        if image_height >= 2 {
            let image_area = Rect::new(inner.x, area.y + image_y, inner.width, image_height);
            render_preview(frame, image_area, post, theme, images, spinner);
        }
    }

    // Footer
    if layout.footer_y >= content_bottom {
        return;
    }
    let footer = Rect::new(inner.x, area.y + layout.footer_y, inner.width, 1);
    let right = footer_badges(post, theme);
    let right_width = width(&right) as u16;
    let left_width = inner.width.saturating_sub(right_width.saturating_add(1));

    let mut left_spans = vec![
        Span::styled(
            format!("▲ {}", format_count(post.score)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            format!("◍ {}", format_count(post.num_comments as i64)),
            Style::default().fg(theme.text_dim),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            post.content.kind_label().to_string(),
            Style::default().fg(theme.badge_fg).bg(theme.badge_bg),
        ),
    ];
    if let Some(flair) = &post.flair
        && !flair.is_empty()
        && inner.width > 50
    {
        left_spans.push(Span::raw(" "));
        left_spans.push(Span::styled(
            format!(" {} ", crate::utils::text::sanitize(flair)),
            Style::default().fg(theme.info).bg(theme.surface_alt),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(left_spans))
            .style(Style::default().fg(theme.muted))
            .wrap(Wrap { trim: true }),
        Rect::new(footer.x, footer.y, left_width, 1),
    );
    if right_width > 0 {
        frame.render_widget(
            Paragraph::new(right).alignment(ratatui::layout::Alignment::Right),
            Rect::new(footer.x + left_width + 1, footer.y, right_width, 1),
        );
    }
}

fn post_meta_line(post: &Post, theme: &Theme, settings: &Settings) -> Line<'static> {
    let time = if settings.ui.relative_time {
        relative_now(post.created_utc)
    } else {
        absolute(post.created_utc)
    };
    let author = crate::utils::text::sanitize(&post.author);
    Line::from(vec![
        Span::styled(
            format!("r/{}", crate::utils::text::sanitize(&post.subreddit)),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  •  ", Style::default().fg(theme.muted)),
        Span::styled(format!("u/{author}"), Style::default().fg(theme.text_dim)),
        Span::styled("  •  ", Style::default().fg(theme.muted)),
        Span::styled(time, Style::default().fg(theme.muted)),
    ])
}

fn footer_badges(post: &Post, theme: &Theme) -> String {
    let mut badges = Vec::new();
    if post.stickied {
        badges.push("PINNED");
    }
    if post.locked {
        badges.push("LOCKED");
    }
    if post.nsfw {
        badges.push("NSFW");
    }
    if post.spoiler {
        badges.push("SPOILER");
    }
    if let PostContent::Gallery { items } = &post.content {
        badges.push(if items.len() > 1 { "GALLERY" } else { "IMAGE" });
    }
    let _ = theme;
    badges.join(" · ")
}

fn render_preview(
    frame: &mut Frame,
    area: Rect,
    post: &Post,
    theme: &Theme,
    images: &mut ImageManager,
    spinner: usize,
) {
    // Clean background first, so stale image content is never left behind.
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize))
            .style(Style::default().bg(theme.surface_alt))
            .wrap(Wrap { trim: false }),
        area,
    );

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

    let status = images.poll(url, area.width, area.height);
    match status {
        ImageStatus::Ready(protocol) => {
            let size = protocol.size();
            let w = size.width.min(area.width);
            let h = size.height.min(area.height);
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let target = Rect::new(x, y, w, h);
            frame.render_widget(
                ratatui_image::Image::new(protocol).allow_clipping(true),
                target,
            );
        }
        ImageStatus::Loading | ImageStatus::NotRequested => {
            let text = format!("{}  loading image…", crate::ui::spinner_char(spinner));
            frame.render_widget(
                Paragraph::new(text)
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(Style::default().fg(theme.muted).bg(theme.surface_alt)),
                area,
            );
        }
        ImageStatus::Failed(error) => {
            let message = ellipsize(
                &format!("Image unavailable: {}", crate::utils::text::sanitize(error)),
                area.width as usize,
            );
            frame.render_widget(
                Paragraph::new(message)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaItem, PostContent};

    fn post(content: PostContent) -> Post {
        Post {
            id: "x".into(),
            subreddit: "rust".into(),
            author: "alice".into(),
            title: "A fairly long title that should wrap over several lines in narrow cards".into(),
            body: "Body text ".repeat(30),
            created_utc: 0,
            score: 1234,
            num_comments: 56,
            permalink: "/r/rust/comments/x".into(),
            content,
            flair: Some("Discussion".into()),
            nsfw: false,
            spoiler: false,
            stickied: false,
            locked: false,
            removed: false,
            is_self: true,
            domain: None,
            thumbnail: None,
        }
    }

    #[test]
    fn card_grows_with_content() {
        let text = card_layout(&post(PostContent::Text), 70, false, true, false);
        let image = card_layout(
            &post(PostContent::Image {
                url: "demo://img/a.png".into(),
            }),
            70,
            false,
            true,
            false,
        );
        assert!(image.total_height > text.total_height);
    }

    #[test]
    fn compact_cards_are_smaller() {
        let normal = card_layout(
            &post(PostContent::Image {
                url: "demo://img/a.png".into(),
            }),
            70,
            false,
            true,
            false,
        );
        let compact = card_layout(
            &post(PostContent::Image {
                url: "demo://img/a.png".into(),
            }),
            70,
            true,
            true,
            false,
        );
        assert!(compact.total_height < normal.total_height);
    }

    #[test]
    fn compact_text_card_fits_two_per_small_viewport() {
        let mut post = post(PostContent::Text);
        post.title = "Short title".into();
        let layout = card_layout(&post, 60, true, true, false);
        assert!(
            layout.total_height <= 7,
            "compact text card should be at most 7 rows, got {}",
            layout.total_height
        );
    }

    #[test]
    fn hidden_cards_hide_image_and_body() {
        let layout = card_layout(
            &post(PostContent::Image {
                url: "demo://img/a.png".into(),
            }),
            70,
            false,
            true,
            true,
        );
        assert!(layout.image_y.is_none());
        assert!(layout.body_y.is_none());
        assert!(layout.note_y.is_some());
    }

    #[test]
    fn gallery_layout_uses_preview() {
        let layout = card_layout(
            &post(PostContent::Gallery {
                items: vec![MediaItem::new("a"), MediaItem::new("b")],
            }),
            90,
            false,
            true,
            false,
        );
        assert!(layout.image_y.is_some());
    }
}
