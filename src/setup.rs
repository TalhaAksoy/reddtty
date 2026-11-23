//! First-run configuration picker.
//!
//! This is intentionally a small standalone screen: it runs before the main App
//! is constructed, writes the selected preset, and then the normal startup path
//! reloads the new config.

use std::io;

use ratatui::DefaultTerminal;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use reddtty::config::presets;
use reddtty::ui::layout;
use reddtty::ui::theme::Theme;

pub fn run(terminal: &mut DefaultTerminal) -> io::Result<Option<&'static str>> {
    let theme = Theme::dark();
    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| render(frame, selected, &theme))?;
        match crossterm::event::read()? {
            crossterm::event::Event::Key(key)
                if key.kind != crossterm::event::KeyEventKind::Release =>
            {
                match key.code {
                    crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                        selected = (selected + 1).min(presets::PRESETS.len() - 1);
                    }
                    crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                        selected = selected.saturating_sub(1);
                    }
                    crossterm::event::KeyCode::Enter => {
                        return Ok(Some(presets::PRESETS[selected].id));
                    }
                    crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                        return Ok(None);
                    }
                    _ => {}
                }
            }
            crossterm::event::Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn render(frame: &mut ratatui::Frame, selected: usize, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    let width = area.width.saturating_sub(4).min(78);
    let height = (presets::PRESETS.len() as u16 * 3 + 7)
        .min(area.height.saturating_sub(2))
        .max(10);
    let rect = layout::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Welcome to ReddTTY ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.elevated));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "No config found.",
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Choose a starter config. You can change it later from Settings.",
                Style::default().fg(theme.text_dim),
            )),
            Line::from(""),
        ])
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(theme.elevated)),
        Rect::new(inner.x, inner.y, inner.width, inner.height.min(3)),
    );

    let list_y = inner.y.saturating_add(3);
    for (index, preset) in presets::PRESETS.iter().enumerate() {
        let row = list_y.saturating_add((index * 3) as u16);
        if row >= inner.bottom().saturating_sub(2) {
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
            theme.text
        };
        let marker = if is_selected { "▸ " } else { "  " };
        frame.render_widget(
            Paragraph::new(format!("{marker}{}", preset.name)).style(
                Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, row, inner.width, 1),
        );
        frame.render_widget(
            Paragraph::new(format!("  {}", preset.description))
                .style(Style::default().fg(theme.muted).bg(bg)),
            Rect::new(inner.x, row.saturating_add(1), inner.width, 1),
        );
    }

    frame.render_widget(
        Paragraph::new("j/k choose · Enter install · Esc use demo for this session")
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}
