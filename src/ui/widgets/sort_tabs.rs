use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::feed::FeedView;
use crate::app::hit::{HitMap, HitTarget};
use crate::providers::ContentProvider;
use crate::ui::theme::Theme;
use crate::utils::text::width;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    feed: &FeedView,
    provider: &dyn ContentProvider,
    theme: &Theme,
    hit_map: &mut HitMap,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let supported = provider.supported_sorts(&feed.spec);
    let mut spans = Vec::new();
    let mut x = area.x;
    let y = area.y;

    for sort in supported.iter().copied() {
        let selected = sort == feed.sort;
        let label = format!(" {} ", sort.label());
        let text_width = width(&label) as u16;
        if x.saturating_add(text_width) > area.right() {
            break;
        }
        let style = if selected {
            Style::default()
                .fg(theme.on_accent)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
        hit_map.push(Rect::new(x, y, text_width, 1), HitTarget::FeedSort(sort));
        x = x.saturating_add(text_width + 1);
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.bg)),
        Rect::new(area.x, y, area.width, 1),
    );
}
