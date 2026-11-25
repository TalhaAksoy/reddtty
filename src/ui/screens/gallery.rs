use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::gallery::GalleryView;
use crate::app::hit::{HitMap, HitTarget, Pane};
use crate::config::Settings;
use crate::media::renderer::{ImageManager, ImageStatus};
use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, sanitize};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    gallery: &GalleryView,
    settings: &Settings,
    images: &mut ImageManager,
    spinner_index: usize,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    let _ = settings;
    let rects = layout::gallery(area, gallery.images.len() > 1);

    // Header
    frame.render_widget(
        Paragraph::new("").style(theme.surface_alt_style()),
        rects.header,
    );
    let title = ellipsize(
        &sanitize(&gallery.title()),
        rects.header.width.saturating_sub(38) as usize,
    );
    let mut spans = vec![
        Span::styled(
            " Image Viewer ",
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} / {}", gallery.index + 1, gallery.images.len()),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(title, Style::default().fg(theme.text_dim)),
    ];
    if gallery.view.zoom > 100 {
        spans.push(Span::styled(
            format!("   Zoom {}  {}", gallery.zoom_label(), gallery.pan_label()),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ));
    }
    if rects.header.width > 50 {
        spans.push(Span::styled(
            format!(
                "   r/{}",
                crate::utils::text::sanitize(&gallery.post.subreddit)
            ),
            Style::default().fg(theme.muted),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme.surface_alt_style()),
        rects.header,
    );

    // Image canvas
    frame.render_widget(Paragraph::new("").style(theme.base()), rects.image);
    hit_map.push(rects.image, HitTarget::Pane(Pane::Gallery));
    let target = Rect::new(
        rects.image.x + 1,
        rects.image.y + 1,
        rects.image.width.saturating_sub(2).max(1),
        rects.image.height.saturating_sub(2).max(1),
    );

    let Some(current) = gallery.current() else {
        frame.render_widget(
            Paragraph::new("No image available.")
                .style(Style::default().fg(theme.muted).bg(theme.bg))
                .alignment(Alignment::Center),
            target,
        );
        render_footer(frame, rects.footer, gallery, theme);
        return;
    };

    if !images.enabled() {
        frame.render_widget(
            Paragraph::new(
                "Images are disabled.\nRestart with --images or enable images in config.",
            )
            .style(Style::default().fg(theme.muted).bg(theme.bg))
            .alignment(Alignment::Center),
            target,
        );
    } else {
        match images.poll_view(&current.url, target.width, target.height, gallery.view) {
            ImageStatus::Ready(protocol) => {
                let size = protocol.size();
                let w = size.width.min(target.width);
                let h = size.height.min(target.height);
                let x = target.x + target.width.saturating_sub(w) / 2;
                let y = target.y + target.height.saturating_sub(h) / 2;
                frame.render_widget(
                    ratatui_image::Image::new(protocol).allow_clipping(true),
                    Rect::new(x, y, w, h),
                );
            }
            ImageStatus::Loading | ImageStatus::NotRequested => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "{}  Loading image…",
                        crate::ui::spinner_char(spinner_index)
                    ))
                    .style(Style::default().fg(theme.muted).bg(theme.bg))
                    .alignment(Alignment::Center),
                    target,
                );
            }
            ImageStatus::Failed(error) => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Image unavailable\n{}\nPress o to open externally, Esc to close.",
                        ellipsize(
                            &crate::utils::text::sanitize(error),
                            target.width.saturating_sub(4) as usize
                        )
                    ))
                    .style(Style::default().fg(theme.danger).bg(theme.bg))
                    .alignment(Alignment::Center),
                    target,
                );
            }
            ImageStatus::Disabled => {
                frame.render_widget(
                    Paragraph::new("Images are disabled.")
                        .style(Style::default().fg(theme.muted).bg(theme.bg))
                        .alignment(Alignment::Center),
                    target,
                );
            }
        }
    }

    // Navigation buttons.
    if gallery.images.len() > 1 {
        let nav_h = 3u16.min(rects.image.height);
        let y = rects.image.y + rects.image.height.saturating_sub(nav_h) / 2;
        let left = Rect::new(rects.image.x, y, 3.min(rects.image.width), nav_h);
        let right = Rect::new(
            rects.image.right().saturating_sub(3.min(rects.image.width)),
            y,
            3.min(rects.image.width),
            nav_h,
        );
        frame.render_widget(
            Paragraph::new("‹")
                .style(Style::default().fg(theme.accent).bg(theme.surface_alt))
                .alignment(Alignment::Center),
            left,
        );
        frame.render_widget(
            Paragraph::new("›")
                .style(Style::default().fg(theme.accent).bg(theme.surface_alt))
                .alignment(Alignment::Center),
            right,
        );
        hit_map.push(left, HitTarget::GalleryPrev);
        hit_map.push(right, HitTarget::GalleryNext);
    }

    if let Some(strip) = rects.thumbs {
        render_thumbnails(frame, strip, gallery, images, spinner_index, theme, hit_map);
    }

    render_footer(frame, rects.footer, gallery, theme);
}

fn render_thumbnails(
    frame: &mut Frame,
    area: Rect,
    gallery: &GalleryView,
    images: &mut ImageManager,
    spinner_index: usize,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    frame.render_widget(Paragraph::new("").style(theme.surface), area);
    let cells = layout::gallery_thumbnails(area, gallery.images.len(), gallery.index);
    for cell in cells {
        let Some(item) = gallery.images.get(cell.index) else {
            continue;
        };
        let selected = cell.index == gallery.index;
        let bg = if selected {
            theme.selected_bg
        } else {
            theme.bg
        };
        frame.render_widget(Paragraph::new("").style(Style::default().bg(bg)), cell.area);
        frame.render_widget(
            Paragraph::new("").style(Style::default().bg(theme.bg)),
            cell.image,
        );
        if images.enabled() {
            match images.poll(&item.url, cell.image.width, cell.image.height) {
                ImageStatus::Ready(protocol) => {
                    let size = protocol.size();
                    let w = size.width.min(cell.image.width);
                    let h = size.height.min(cell.image.height);
                    if w > 0 && h > 0 {
                        frame.render_widget(
                            ratatui_image::Image::new(protocol).allow_clipping(true),
                            Rect::new(
                                cell.image.x + cell.image.width.saturating_sub(w) / 2,
                                cell.image.y + cell.image.height.saturating_sub(h) / 2,
                                w,
                                h,
                            ),
                        );
                    }
                }
                ImageStatus::Loading if selected => {
                    frame.render_widget(
                        Paragraph::new(crate::ui::spinner_char(spinner_index))
                            .style(Style::default().fg(theme.accent).bg(theme.bg))
                            .alignment(Alignment::Center),
                        cell.image,
                    );
                }
                ImageStatus::Failed(_) => {
                    frame.render_widget(
                        Paragraph::new("×")
                            .style(Style::default().fg(theme.danger).bg(theme.bg))
                            .alignment(Alignment::Center),
                        cell.image,
                    );
                }
                _ => {}
            }
        }
        let label = format!("{}", cell.index + 1);
        frame.render_widget(
            Paragraph::new(label)
                .style(
                    Style::default()
                        .fg(if selected { theme.accent } else { theme.muted })
                        .bg(bg)
                        .add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                )
                .alignment(Alignment::Center),
            Rect::new(cell.area.x, cell.area.y + 4, cell.area.width, 1),
        );
        hit_map.push(cell.area, HitTarget::GalleryThumb(cell.index));
    }
    frame.render_widget(
        Paragraph::new(" thumbnails · click to jump ")
            .style(Style::default().fg(theme.muted).bg(theme.surface))
            .alignment(Alignment::Right),
        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 1),
    );
}

fn render_footer(frame: &mut Frame, area: Rect, gallery: &GalleryView, theme: &Theme) {
    if area.height == 0 {
        return;
    }
    let hint = if area.width >= 100 {
        format!(
            " ←/→ image · +/− zoom · Shift+arrows pan · 0 fit · o open · Esc close        {} / {} · {} ",
            gallery.index + 1,
            gallery.images.len(),
            gallery.zoom_label()
        )
    } else if area.width >= 56 {
        format!(
            " ←/→ image · +/− zoom · 0 fit · o open · Esc close    {} / {} ",
            gallery.index + 1,
            gallery.images.len()
        )
    } else {
        format!(
            "←/→ · +/− · Esc    {} / {}",
            gallery.index + 1,
            gallery.images.len()
        )
    };
    let hint = ellipsize(&hint, area.width as usize);
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(theme.text_dim).bg(theme.surface_alt)),
        area,
    );
}
