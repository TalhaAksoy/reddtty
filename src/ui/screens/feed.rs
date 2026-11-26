use std::collections::HashSet;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::Focus;
use crate::app::feed::FeedView;
use crate::app::hit::{HitMap, HitTarget, Pane};
use crate::app::sidebar::{SUGGESTED, SidebarState};
use crate::config::Settings;
use crate::media::renderer::ImageManager;
use crate::providers::ContentProvider;
use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::ui::widgets::{draw_scrollbar, post_card, sidebar, sort_tabs};
use crate::utils::text::{ellipsize, format_count, width, wrap_text};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    feed: &FeedView,
    settings: &Settings,
    focus: Focus,
    images: &mut ImageManager,
    sidebar_state: &SidebarState,
    provider: &dyn ContentProvider,
    demo: bool,
    spinner: &str,
    spinner_index: usize,
    revealed: &HashSet<String>,
    library: &crate::library::Library,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let body_layout = layout::feed(area, settings, true);

    if let Some(sidebar_area) = body_layout.sidebar {
        sidebar::render(
            frame,
            sidebar_area,
            settings,
            provider.capabilities(),
            sidebar_state,
            Some(&feed.spec),
            focus,
            theme,
            hit_map,
        );
    }
    if let Some(right) = body_layout.right {
        render_right_panel(frame, right, feed, provider, spinner, theme);
    }

    let feed_area = body_layout.feed;
    let lists = layout::feed_list(feed_area);
    let (cards_area, scrollbar) = layout::cards_and_scrollbar(lists.list);

    render_feed_header(
        frame,
        lists.header,
        feed,
        provider,
        demo,
        spinner,
        theme,
        hit_map,
    );

    hit_map.push(lists.list, HitTarget::Pane(Pane::FeedList));

    if feed.posts.is_empty() {
        render_feed_state(frame, lists.list, feed, spinner, theme, hit_map);
        return;
    }

    let compact = post_card::compact_for_viewport(lists.list.height);
    let mut last_bottom = lists.list.y;

    let status_rows = u16::from(feed.more_error.is_some() || feed.loading_more);
    let list_height = lists.list.height.saturating_sub(status_rows);
    let viewport_top = feed.offset;
    let viewport_bottom = feed.offset.saturating_add(list_height as u32);
    for index in feed.visible_range(list_height) {
        let Some(post) = feed.posts.get(index) else {
            break;
        };
        let full_height = feed.heights.get(index).copied().unwrap_or(12);
        let card_start = feed.prefix_height(index);
        let card_end = card_start.saturating_add(full_height as u32);
        let visible_top = card_start.max(viewport_top);
        let visible_bottom = card_end.min(viewport_bottom);
        if visible_bottom <= visible_top {
            continue;
        }
        let dest_y = lists
            .list
            .y
            .saturating_add((visible_top - viewport_top) as u16);
        let dest_height = (visible_bottom - visible_top) as u16;
        if dest_height == 0 {
            continue;
        }
        let dest = Rect::new(cards_area.x, dest_y, cards_area.width, dest_height);
        let selected = index == feed.selected && focus == Focus::Feed;
        if card_start >= viewport_top && card_end <= viewport_bottom {
            // Fully visible: paint directly, no temporary buffer.
            post_card::render(
                frame,
                dest,
                post,
                selected,
                theme,
                settings,
                images,
                compact,
                revealed.contains(&post.id),
                spinner_index,
                !library.is_post_read(&post.id),
                library.is_bookmarked(&post.id),
            );
        } else {
            // Partially visible: render the whole card into an off-screen
            // buffer, then copy only the visible fragment. Borders and images
            // stay inside the card and cannot paint over the feed header.
            let full_area = Rect::new(0, 0, cards_area.width, full_height);
            let mut buffer = Buffer::empty(full_area);
            post_card::render_buffer(
                &mut buffer,
                full_area,
                post,
                selected,
                theme,
                settings,
                images,
                compact,
                revealed.contains(&post.id),
                spinner_index,
                !library.is_post_read(&post.id),
                library.is_bookmarked(&post.id),
            );
            let source_y = (visible_top - card_start) as u16;
            let target = frame.buffer_mut();
            for row in 0..dest_height {
                for column in 0..cards_area.width {
                    target[(dest.x + column, dest.y + row)] =
                        buffer[(column, source_y + row)].clone();
                }
            }
        }
        hit_map.push(dest, HitTarget::FeedPost(index));
        last_bottom = dest_y.saturating_add(dest_height);
    }

    if feed.loading_more {
        let y = if last_bottom.saturating_add(1) < lists.list.bottom() {
            last_bottom.saturating_add(1)
        } else {
            lists.list.bottom().saturating_sub(1)
        };
        if y >= lists.list.y {
            frame.render_widget(
                Paragraph::new(format!("{spinner}  Loading more posts…"))
                    .style(Style::default().fg(theme.accent).bg(theme.bg))
                    .alignment(ratatui::layout::Alignment::Center),
                Rect::new(cards_area.x, y, cards_area.width, 1),
            );
        }
    }

    if let Some(error) = &feed.more_error {
        let y = if last_bottom.saturating_add(1) < lists.list.bottom() {
            last_bottom.saturating_add(1)
        } else {
            lists.list.bottom().saturating_sub(1)
        };
        if y >= lists.list.y {
            let hint = feed
                .retry_wait_secs()
                .map(|seconds| format!("rate limited · retry in {seconds}s"))
                .unwrap_or_else(|| "press R to retry".into());
            let text = ellipsize(
                &format!(
                    "Could not load more: {}  ·  {hint}",
                    crate::utils::text::sanitize(error)
                ),
                cards_area.width as usize,
            );
            frame.render_widget(
                Paragraph::new(text)
                    .style(
                        Style::default()
                            .fg(theme.danger)
                            .bg(theme.bg)
                            .add_modifier(Modifier::BOLD),
                    )
                    .alignment(ratatui::layout::Alignment::Center),
                Rect::new(cards_area.x, y, cards_area.width, 1),
            );
            hit_map.push(
                Rect::new(cards_area.x, y, cards_area.width, 1),
                HitTarget::RetryMore,
            );
        }
    }

    if let Some(rect) = scrollbar {
        let (position, total, viewport) = feed.scrollbar(lists.list.height);
        draw_scrollbar(frame, rect, position, total, viewport, theme, theme.bg);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_feed_header(
    frame: &mut Frame,
    area: Rect,
    feed: &FeedView,
    provider: &dyn ContentProvider,
    demo: bool,
    spinner: &str,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(Paragraph::new("").style(theme.base()), area);

    let raw_title = match (&feed.spec, &feed.subreddit) {
        (_, Some(sub)) if !sub.title.is_empty() && feed.spec.subreddit_name().is_some() => {
            format!(
                "{}  ·  {}",
                feed.spec.title(),
                crate::utils::text::sanitize(&sub.title)
            )
        }
        _ => feed.spec.title(),
    };
    let title = crate::utils::text::sanitize(&raw_title);
    let meta = feed_meta(feed);
    let meta_width = width(&meta) as u16;
    let show_meta = area.width > meta_width.saturating_add(8);
    let title_width = if show_meta {
        area.width.saturating_sub(meta_width + 2)
    } else {
        area.width
    };
    let mut title_spans = vec![Span::styled(
        ellipsize(&title, title_width as usize),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    )];
    if feed.spec.is_search() {
        title_spans.push(Span::styled(
            "  search results",
            Style::default().fg(theme.muted),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(title_spans)).style(theme.base()),
        Rect::new(area.x, area.y, title_width, 1),
    );
    if show_meta {
        frame.render_widget(
            Paragraph::new(meta)
                .style(Style::default().fg(theme.muted).bg(theme.bg))
                .alignment(ratatui::layout::Alignment::Right),
            Rect::new(
                area.right().saturating_sub(meta_width),
                area.y,
                meta_width,
                1,
            ),
        );
    }

    if area.height >= 2 {
        sort_tabs::render(
            frame,
            Rect::new(area.x, area.y + 1, area.width, 1),
            feed,
            provider,
            theme,
            hit_map,
        );
    }
    if area.height >= 3 {
        let row = Rect::new(area.x, area.y + 2, area.width, 1);
        if let Some(error) = &feed.error {
            let error = crate::utils::text::sanitize(error);
            frame.render_widget(
                Paragraph::new(ellipsize(&error, area.width as usize))
                    .style(Style::default().fg(theme.danger).bg(theme.bg)),
                row,
            );
        } else if feed.loading_initial {
            frame.render_widget(
                Paragraph::new(format!("{spinner}  Loading posts…"))
                    .style(Style::default().fg(theme.accent).bg(theme.bg)),
                row,
            );
        } else if feed.refreshing {
            frame.render_widget(
                Paragraph::new(format!("{spinner}  Refreshing…"))
                    .style(Style::default().fg(theme.muted).bg(theme.bg)),
                row,
            );
        } else {
            let description = feed
                .subreddit
                .as_ref()
                .map(|s| s.description.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| match &feed.spec {
                    crate::models::FeedSpec::Home => {
                        "Your front page · choose a sort or open a community from the sidebar."
                            .into()
                    }
                    crate::models::FeedSpec::Popular => {
                        "The most popular posts across Reddit right now.".into()
                    }
                    crate::models::FeedSpec::Search { .. } => {
                        if demo {
                            "Results from the embedded demo index.".into()
                        } else {
                            "Results from Reddit search.".into()
                        }
                    }
                    crate::models::FeedSpec::Subreddit(_) => String::new(),
                });
            frame.render_widget(
                Paragraph::new(ellipsize(&description, area.width as usize))
                    .style(Style::default().fg(theme.muted).bg(theme.bg)),
                row,
            );
        }
    }
}

fn feed_meta(feed: &FeedView) -> String {
    let mut meta = if feed.sort == crate::models::FeedSort::Top {
        format!(
            "Top · {} · {} posts",
            feed.top_range.label(),
            feed.posts.len()
        )
    } else {
        format!("{} posts", feed.posts.len())
    };
    if feed.loading_more {
        meta.push_str(" · loading…");
    } else if feed.has_more {
        meta.push_str(" · more");
    }
    meta
}

fn render_feed_state(
    frame: &mut Frame,
    area: Rect,
    feed: &FeedView,
    spinner: &str,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(Paragraph::new("").style(theme.base()), area);
    if let Some(error) = &feed.error {
        let width = area.width.saturating_sub(4).min(90);
        let max_error_lines = area.height.saturating_sub(4).max(1) as usize;
        let mut error_lines = wrap_text(
            &crate::utils::text::sanitize(error),
            width.saturating_sub(2) as usize,
        );
        if error_lines.len() > max_error_lines {
            error_lines.truncate(max_error_lines.saturating_sub(1));
            error_lines.push("…".into());
        }
        let mut lines = vec![
            Line::from(Span::styled(
                "Unable to load posts.",
                Style::default()
                    .fg(theme.danger)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        lines.extend(
            error_lines
                .into_iter()
                .map(|line| Line::from(Span::styled(line, Style::default().fg(theme.text_dim)))),
        );
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "[ Retry ]  press r",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        let height = (lines.len() as u16).min(area.height);
        let rect = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        let retry_y = rect.y.saturating_add(lines.len().saturating_sub(1) as u16);
        frame.render_widget(
            Paragraph::new(lines)
                .style(theme.base())
                .alignment(ratatui::layout::Alignment::Center),
            rect,
        );
        hit_map.push(Rect::new(rect.x, retry_y, rect.width, 1), HitTarget::Retry);
        return;
    }
    if feed.loading_initial || (feed.posts.is_empty() && feed.loading_more) {
        let label = if feed.loading_initial {
            "Loading posts…"
        } else {
            "Loading more posts…"
        };
        frame.render_widget(
            Paragraph::new(format!("{spinner}  {label}"))
                .style(Style::default().fg(theme.muted).bg(theme.bg))
                .alignment(ratatui::layout::Alignment::Center),
            Rect::new(area.x, area.y + area.height / 2, area.width, 1),
        );
        return;
    }
    let lines = vec![
        Line::from(Span::styled(
            "No posts found.",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Try another subreddit or sorting option.",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "g go to subreddit   ·   s change sort   ·   r refresh",
            Style::default().fg(theme.text_dim),
        )),
    ];
    let height = (lines.len() as u16).min(area.height);
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.base())
            .alignment(ratatui::layout::Alignment::Center),
        Rect::new(
            area.x,
            area.y + area.height.saturating_sub(height) / 2,
            area.width,
            height,
        ),
    );
}

fn render_right_panel(
    frame: &mut Frame,
    area: Rect,
    feed: &FeedView,
    provider: &dyn ContentProvider,
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
    lines.push(Line::from(Span::styled(
        "ABOUT",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        crate::utils::text::sanitize(&feed.spec.title()),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));

    if let Some(sub) = &feed.subreddit {
        lines.push(Line::from(Span::styled(
            format!(
                "{} members  ·  {} online",
                format_count(sub.subscribers as i64),
                format_count(sub.active_users as i64)
            ),
            Style::default().fg(theme.text_dim),
        )));
        lines.push(Line::from(""));
        for wrapped in wrap_text(
            &crate::utils::text::sanitize(&sub.description),
            inner.width.saturating_sub(2) as usize,
        )
        .into_iter()
        .take(6)
        {
            lines.push(Line::from(Span::styled(
                wrapped,
                Style::default().fg(theme.text_dim),
            )));
        }
    } else if let Some(error) = &feed.subreddit_error {
        lines.push(Line::from(Span::styled(
            ellipsize(
                &crate::utils::text::sanitize(error),
                inner.width.saturating_sub(2) as usize,
            ),
            Style::default().fg(theme.danger),
        )));
    } else if feed.spec.subreddit_name().is_some() {
        lines.push(Line::from(Span::styled(
            format!("{spinner} loading community…"),
            Style::default().fg(theme.muted),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            match provider.short_name() {
                "DEMO" => "Demo mode: synthetic posts and comments, stored locally.",
                "OLD REDDIT" => "HTML mode: Old Reddit pages, not the official API.",
                "REDLIB" => "HTML mode: your Redlib instance, not an official Reddit API.",
                "REDDIT" => "Live mode: read-only Reddit OAuth API.",
                _ => "Read-only provider mode; the active source is shown in the top bar.",
            },
            Style::default().fg(theme.text_dim),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "SUGGESTED COMMUNITIES",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));
    for name in SUGGESTED.iter().take(6) {
        lines.push(Line::from(vec![
            Span::styled("▸ ", Style::default().fg(theme.accent)),
            Span::styled(format!("r/{name}"), Style::default().fg(theme.text_dim)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "SHORTCUTS",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));
    for (key, desc) in [
        ("j/k", "navigate"),
        ("Enter", "open post"),
        ("s", "sort"),
        ("t", "top range"),
        ("g", "subreddit"),
        ("/", "search"),
        ("r", "refresh"),
        ("o", "open browser"),
        ("v", "image viewer"),
        ("x", "reveal hidden"),
        ("b", "bookmark"),
        ("m", "read / unread"),
        ("i", "community info"),
        ("?", "help"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("{key:<6}"), Style::default().fg(theme.accent)),
            Span::styled(desc.to_string(), Style::default().fg(theme.text_dim)),
        ]));
    }
    let max = inner.height as usize;
    lines.truncate(max);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.surface).fg(theme.text))
            .wrap(Wrap { trim: false }),
        inner,
    );
}
