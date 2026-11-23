//! Central colour and style system. No widget should hard-code colours.

use ratatui::style::{Color, Modifier, Style};

use crate::config::ThemeMode;
use crate::utils::markdown::MdStyles;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub elevated: Color,
    pub text: Color,
    pub text_dim: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub on_accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub border: Color,
    pub border_active: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub badge_bg: Color,
    pub badge_fg: Color,
    pub code_bg: Color,
    pub scroll_track: Color,
    pub scroll_thumb: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: Color::Rgb(13, 15, 18),
            surface: Color::Rgb(21, 24, 29),
            surface_alt: Color::Rgb(27, 31, 37),
            elevated: Color::Rgb(35, 40, 48),
            text: Color::Rgb(232, 234, 238),
            text_dim: Color::Rgb(178, 183, 192),
            muted: Color::Rgb(122, 128, 139),
            accent: Color::Rgb(255, 92, 20),
            accent_soft: Color::Rgb(84, 43, 24),
            on_accent: Color::Rgb(28, 15, 6),
            success: Color::Rgb(92, 203, 132),
            warning: Color::Rgb(242, 192, 86),
            danger: Color::Rgb(242, 98, 98),
            info: Color::Rgb(104, 174, 245),
            border: Color::Rgb(46, 52, 62),
            border_active: Color::Rgb(255, 92, 20),
            selected_bg: Color::Rgb(52, 37, 28),
            selected_fg: Color::Rgb(255, 231, 214),
            badge_bg: Color::Rgb(43, 49, 59),
            badge_fg: Color::Rgb(214, 219, 227),
            code_bg: Color::Rgb(33, 38, 46),
            scroll_track: Color::Rgb(36, 41, 49),
            scroll_thumb: Color::Rgb(89, 97, 110),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color::Rgb(249, 249, 247),
            surface: Color::Rgb(255, 255, 253),
            surface_alt: Color::Rgb(239, 240, 236),
            elevated: Color::Rgb(255, 255, 255),
            text: Color::Rgb(27, 29, 33),
            text_dim: Color::Rgb(68, 72, 80),
            muted: Color::Rgb(112, 117, 126),
            accent: Color::Rgb(214, 62, 2),
            accent_soft: Color::Rgb(255, 226, 205),
            on_accent: Color::Rgb(255, 250, 244),
            success: Color::Rgb(22, 133, 69),
            warning: Color::Rgb(158, 105, 0),
            danger: Color::Rgb(190, 42, 42),
            info: Color::Rgb(28, 105, 190),
            border: Color::Rgb(209, 211, 206),
            border_active: Color::Rgb(214, 62, 2),
            selected_bg: Color::Rgb(255, 231, 211),
            selected_fg: Color::Rgb(77, 28, 0),
            badge_bg: Color::Rgb(232, 233, 228),
            badge_fg: Color::Rgb(55, 59, 66),
            code_bg: Color::Rgb(236, 237, 233),
            scroll_track: Color::Rgb(226, 227, 222),
            scroll_thumb: Color::Rgb(150, 154, 160),
        }
    }

    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    pub fn surface_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.surface)
    }

    pub fn surface_alt_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.surface_alt)
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn accent(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn accent_plain(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn danger(&self) -> Style {
        Style::default().fg(self.danger)
    }

    pub fn info(&self) -> Style {
        Style::default().fg(self.info)
    }

    pub fn badge(&self) -> Style {
        Style::default().fg(self.badge_fg).bg(self.badge_bg)
    }

    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.selected_fg)
            .bg(self.selected_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn card(&self, selected: bool) -> Style {
        if selected {
            Style::default().fg(self.text).bg(self.selected_bg)
        } else {
            Style::default().fg(self.text).bg(self.surface)
        }
    }

    pub fn border(&self, selected: bool) -> Style {
        Style::default()
            .fg(if selected {
                self.border_active
            } else {
                self.border
            })
            .bg(if selected {
                self.selected_bg
            } else {
                self.surface
            })
    }

    pub fn md_styles(&self) -> MdStyles {
        MdStyles {
            text: Style::default().fg(self.text),
            heading: Style::default().fg(self.text).add_modifier(Modifier::BOLD),
            strong: Style::default().fg(self.text).add_modifier(Modifier::BOLD),
            emphasis: Style::default()
                .fg(self.text_dim)
                .add_modifier(Modifier::ITALIC),
            code: Style::default().fg(self.warning).bg(self.code_bg),
            link: Style::default()
                .fg(self.info)
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(self.text_dim)
                .add_modifier(Modifier::ITALIC),
            bullet: Style::default().fg(self.accent),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
