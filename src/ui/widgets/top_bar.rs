use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::ui::theme::Theme;
use crate::utils::text::{ellipsize, width};

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    provider_label: &str,
    demo: bool,
    provider_status: Option<&str>,
    loading: bool,
    spinner: &str,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    frame.render_widget(Paragraph::new("").style(theme.surface_alt_style()), area);

    let label = crate::utils::text::sanitize(provider_label);
    let short_label = label.split(" · ").next().unwrap_or(&label);
    let mode = format!("● {label}");
    let short_mode = format!("● {short_label}");
    let status = provider_status.map(crate::utils::text::sanitize);
    let mode_style = if demo {
        theme.warning()
    } else {
        theme.success()
    };
    let spinner_prefix = if loading {
        format!("{spinner}  ")
    } else {
        " ".to_string()
    };
    // Choose the richest badge that leaves room for the view title. Status is
    // more important than the help affordance, and the provider badge survives
    // at narrow widths so demo content never looks live.
    let candidates = if let Some(status) = &status {
        vec![
            format!("{spinner_prefix}{mode} — {status}   ? Help "),
            format!("{spinner_prefix}{mode} — {status} "),
            format!("{spinner_prefix}{short_mode} — {status}   ? Help "),
            format!("{spinner_prefix}{short_mode} — {status} "),
            format!("{spinner_prefix}{mode}   ? Help "),
            format!("{spinner_prefix}{short_mode}   ? Help "),
            format!("{spinner_prefix}{mode} "),
            format!("{spinner_prefix}{short_mode} "),
        ]
    } else {
        vec![
            format!("{spinner_prefix}{mode}   ? Help "),
            format!("{spinner_prefix}{short_mode}   ? Help "),
            format!("{spinner_prefix}{mode} "),
            format!("{spinner_prefix}{short_mode} "),
        ]
    };
    let right = candidates
        .into_iter()
        .find(|candidate| width(candidate) + 16 <= area.width as usize)
        .unwrap_or_else(|| format!("{mode} "));
    let right_width = width(&right) as u16;
    let show_right = !right.is_empty() && area.width > right_width;
    let show_help = right.contains("? Help");
    let show_status = status.is_some() && right.contains('—');
    let selected_mode = if right.contains(&mode) {
        mode.clone()
    } else {
        short_mode
    };

    let brand = Span::styled(
        " REDDTTY ",
        Style::default()
            .fg(theme.on_accent)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    let title = crate::utils::text::sanitize(title);
    let title_width =
        area.width
            .saturating_sub(13)
            .saturating_sub(if show_right { right_width + 1 } else { 0 }) as usize;
    let title = ellipsize(&title, title_width);

    let left = Line::from(vec![
        brand,
        Span::styled("  ›  ", Style::default().fg(theme.muted)),
        Span::styled(title, Style::default().fg(theme.text_dim)),
    ]);
    frame.render_widget(
        Paragraph::new(left).style(theme.surface_alt_style()),
        Rect::new(
            area.x,
            area.y,
            area.width
                .saturating_sub(if show_right { right_width } else { 0 }),
            1,
        ),
    );

    if show_right && area.width > right_width {
        let [left_area, right_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);
        let _ = left_area;
        let mut spans = Vec::new();
        if loading {
            spans.push(Span::styled(
                format!("{spinner} "),
                Style::default().fg(theme.accent),
            ));
        }
        spans.push(Span::styled(
            selected_mode,
            mode_style.add_modifier(Modifier::BOLD),
        ));
        if show_status {
            spans.push(Span::styled(
                format!(" — {}", status.unwrap_or_default()),
                Style::default().fg(theme.danger),
            ));
        }
        if show_help {
            spans.push(Span::styled("   ? Help ", Style::default().fg(theme.muted)));
        } else {
            spans.push(Span::raw(" "));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans))
                .style(theme.surface_alt_style())
                .alignment(ratatui::layout::Alignment::Right),
            right_area,
        );
    }
}
