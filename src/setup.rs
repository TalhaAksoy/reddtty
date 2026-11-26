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
use reddtty::config::{presets, validate_html_base};
use reddtty::ui::layout;
use reddtty::ui::theme::Theme;

pub enum SetupChoice {
    Preset(&'static str),
    CustomRedlib(String),
}

enum Mode {
    List,
    Custom {
        value: String,
        error: Option<String>,
    },
    ConfirmCustom {
        url: String,
    },
}

pub fn run(terminal: &mut DefaultTerminal) -> io::Result<Option<SetupChoice>> {
    let theme = if std::env::var_os("NO_COLOR").is_some() {
        Theme::no_color()
    } else {
        Theme::dark()
    };
    let mut selected = 0usize;
    let mut mode = Mode::List;
    let last = presets::PRESETS.len();
    loop {
        terminal.draw(|frame| match &mode {
            Mode::List => render(frame, selected, &theme),
            Mode::Custom { value, error } => render_custom(frame, value, error.as_deref(), &theme),
            Mode::ConfirmCustom { url } => render_confirm_custom(frame, url, &theme),
        })?;
        match crossterm::event::read()? {
            crossterm::event::Event::Key(key)
                if key.kind != crossterm::event::KeyEventKind::Release =>
            {
                match &mut mode {
                    Mode::List => match key.code {
                        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                            selected = (selected + 1).min(last);
                        }
                        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                            selected = selected.saturating_sub(1);
                        }
                        crossterm::event::KeyCode::PageDown => {
                            selected = (selected + 3).min(last);
                        }
                        crossterm::event::KeyCode::PageUp => {
                            selected = selected.saturating_sub(3);
                        }
                        crossterm::event::KeyCode::Home => selected = 0,
                        crossterm::event::KeyCode::End => selected = last,
                        crossterm::event::KeyCode::Enter => {
                            if let Some(preset) = presets::PRESETS.get(selected) {
                                return Ok(Some(SetupChoice::Preset(preset.id)));
                            }
                            mode = Mode::Custom {
                                value: String::new(),
                                error: None,
                            };
                        }
                        crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                            return Ok(None);
                        }
                        _ => {}
                    },
                    Mode::Custom { value, error } => match key.code {
                        crossterm::event::KeyCode::Esc => mode = Mode::List,
                        crossterm::event::KeyCode::Backspace => {
                            value.pop();
                            *error = None;
                        }
                        crossterm::event::KeyCode::Char(ch)
                            if !ch.is_control() && value.chars().count() < 200 =>
                        {
                            value.push(ch);
                            *error = None;
                        }
                        crossterm::event::KeyCode::Enter => {
                            let raw = value.trim();
                            let raw = if raw.starts_with("http://") || raw.starts_with("https://") {
                                raw.to_string()
                            } else {
                                format!("https://{raw}")
                            };
                            match validate_html_base(&raw) {
                                Ok(url) => mode = Mode::ConfirmCustom { url },
                                Err(message) => *error = Some(message),
                            }
                        }
                        _ => {}
                    },
                    Mode::ConfirmCustom { url } => match key.code {
                        crossterm::event::KeyCode::Esc => mode = Mode::List,
                        crossterm::event::KeyCode::Enter
                        | crossterm::event::KeyCode::Char('y')
                        | crossterm::event::KeyCode::Char('Y') => {
                            return Ok(Some(SetupChoice::CustomRedlib(url.clone())));
                        }
                        _ => {}
                    },
                }
            }
            crossterm::event::Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

fn render_confirm_custom(frame: &mut ratatui::Frame, url: &str, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    let width = area.width.saturating_sub(4).min(78);
    let height = area.height.saturating_sub(2).clamp(8, 12);
    let rect = layout::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Enable network access? ",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_active))
        .style(Style::default().bg(theme.elevated));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let lines = vec![
        Line::from(Span::styled(
            format!("Use {url} for Redlib?"),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "HTML network requests will be enabled for this source.",
            Style::default().fg(theme.text_dim),
        )),
        Line::from(Span::styled(
            "The instance operator can see your request paths, headers and IP address.",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter/y confirm · Esc back",
            Style::default().fg(theme.accent),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_custom(frame: &mut ratatui::Frame, value: &str, error: Option<&str>, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    let width = area.width.saturating_sub(4).min(78);
    let height = area.height.saturating_sub(2).clamp(9, 14);
    let rect = layout::centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Custom Redlib instance ",
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
    let mut lines = vec![
        Line::from(Span::styled(
            "HTTPS root URL (for example https://redlib.example):",
            Style::default().fg(theme.text_dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(theme.accent)),
            Span::styled(value.to_string(), Style::default().fg(theme.text)),
            Span::styled("▏", Style::default().fg(theme.accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter continues to a network-consent step. Esc goes back.",
            Style::default().fg(theme.muted),
        )),
    ];
    if let Some(error) = error {
        lines.push(Line::from(Span::styled(
            error.to_string(),
            Style::default().fg(theme.danger),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.elevated))
            .wrap(Wrap { trim: false }),
        inner,
    );
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
    if inner.height >= 18 {
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
    }

    let compact = inner.height < 18;
    let intro_height = if compact { 0 } else { 3 };
    let row_height = if compact { 1 } else { 3 };
    let list_height = inner.height.saturating_sub(intro_height).saturating_sub(1);
    let visible = (list_height / row_height).max(1) as usize;
    let total = presets::PRESETS.len() + 1;
    let offset = selected.saturating_sub(visible.saturating_sub(1));
    let list_y = inner.y.saturating_add(intro_height);
    for (slot, index) in (offset..total).take(visible).enumerate() {
        let (name, description) = match presets::PRESETS.get(index) {
            Some(preset) => (preset.name, preset.description),
            None => (
                "+ Custom Redlib instance…",
                "Self-hosted instance; enter its HTTPS root URL next.",
            ),
        };
        let row = list_y.saturating_add((slot * row_height as usize) as u16);
        if row >= inner.bottom().saturating_sub(1) {
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
            Paragraph::new(format!("{marker}{name}")).style(
                Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Rect::new(inner.x, row, inner.width, 1),
        );
        if !compact && row.saturating_add(1) < inner.bottom().saturating_sub(1) {
            frame.render_widget(
                Paragraph::new(format!("  {description}"))
                    .style(Style::default().fg(theme.muted).bg(bg)),
                Rect::new(inner.x, row.saturating_add(1), inner.width, 1),
            );
        }
    }

    let footer = if compact {
        "j/k · Enter · Esc".to_string()
    } else {
        format!(
            "{}–{} / {} · j/k · PgUp/PgDn · Enter · Esc",
            offset + 1,
            (offset + visible).min(total),
            total
        )
    };
    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(theme.muted).bg(theme.elevated))
            .alignment(Alignment::Right),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen_at(selected: usize, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, selected, &Theme::dark()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn setup_fits_the_default_80x24_terminal() {
        let first = screen_at(0, 80, 24);
        assert!(first.contains("Demo (offline)"), "{first}");
        assert!(first.contains("j/k"), "{first}");

        let last = screen_at(presets::PRESETS.len(), 80, 24);
        assert!(last.contains("Custom Redlib instance"), "{last}");
        assert!(last.contains("safereddit"), "{last}");
    }

    #[test]
    fn setup_remains_usable_at_30x10() {
        let text = screen_at(presets::PRESETS.len(), 30, 10);
        assert!(text.contains("Custom Redlib"), "{text}");
    }
}
