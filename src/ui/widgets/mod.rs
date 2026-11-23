pub mod comment_tree;
pub mod post_card;
pub mod sidebar;
pub mod sort_tabs;
pub mod status_bar;
pub mod top_bar;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

use crate::ui::theme::Theme;

/// Draw a vertical scrollbar in a one-column rect.
pub fn draw_scrollbar(
    frame: &mut Frame,
    rect: Rect,
    position: u32,
    total: u32,
    viewport: u16,
    theme: &Theme,
    background: ratatui::style::Color,
) {
    if rect.height == 0 || rect.width == 0 || total == 0 {
        return;
    }
    let track = rect.height as u32;
    let view = viewport.max(1) as u32;
    let thumb = view.saturating_mul(track).div_ceil(total).clamp(1, track);
    let max_pos = total.saturating_sub(view);
    let thumb_start = if max_pos == 0 {
        0
    } else {
        ((position.min(max_pos) as u64 * (track.saturating_sub(thumb)) as u64) / max_pos as u64)
            as u32
    };
    let buf = frame.buffer_mut();
    for row in 0..track {
        if let Some(cell) = buf.cell_mut(Position::new(rect.x, rect.y + row as u16)) {
            if row >= thumb_start && row < thumb_start + thumb {
                cell.set_symbol("┃")
                    .set_fg(theme.scroll_thumb)
                    .set_bg(background);
            } else {
                cell.set_symbol("│")
                    .set_fg(theme.scroll_track)
                    .set_bg(background);
            }
        }
    }
}

/// A one-column accent rail used to show pane focus.
pub fn draw_focus_rail(
    frame: &mut Frame,
    rect: Rect,
    active: bool,
    theme: &Theme,
    background: ratatui::style::Color,
) {
    if rect.height == 0 || rect.width == 0 {
        return;
    }
    let buf = frame.buffer_mut();
    let x = rect.x;
    for y in rect.y..rect.y.saturating_add(rect.height) {
        if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
            if active {
                cell.set_symbol("▌").set_fg(theme.accent).set_bg(background);
            } else {
                cell.set_symbol(" ").set_bg(background);
            }
        }
    }
}

/// Horizontal rule drawn with a dim style.
pub fn draw_rule(frame: &mut Frame, area: Rect, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let text = "─".repeat(area.width as usize);
    frame.render_widget(
        ratatui::widgets::Paragraph::new(text)
            .style(Style::default().fg(theme.border).bg(theme.bg)),
        area,
    );
}
