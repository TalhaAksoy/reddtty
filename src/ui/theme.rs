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

struct CatppuccinPalette {
    base: u32,
    mantle: u32,
    crust: u32,
    surface0: u32,
    surface1: u32,
    surface2: u32,
    text: u32,
    subtext0: u32,
    overlay1: u32,
    peach: u32,
    red: u32,
    green: u32,
    yellow: u32,
    blue: u32,
}

const CATPPUCCIN_LATTE: CatppuccinPalette = CatppuccinPalette {
    base: 0xeff1f5,
    mantle: 0xe6e9ef,
    crust: 0xdce0e8,
    surface0: 0xccd0da,
    surface1: 0xbcc0cc,
    surface2: 0xacb0be,
    text: 0x4c4f69,
    subtext0: 0x6c6f85,
    overlay1: 0x8c8fa1,
    peach: 0xfe640b,
    red: 0xd20f39,
    green: 0x40a02b,
    yellow: 0xdf8e1d,
    blue: 0x1e66f5,
};

const CATPPUCCIN_FRAPPE: CatppuccinPalette = CatppuccinPalette {
    base: 0x303446,
    mantle: 0x292c3c,
    crust: 0x232634,
    surface0: 0x414559,
    surface1: 0x51576d,
    surface2: 0x626880,
    text: 0xc6d0f5,
    subtext0: 0xa5adce,
    overlay1: 0x838ba7,
    peach: 0xef9f76,
    red: 0xe78284,
    green: 0xa6d189,
    yellow: 0xe5c890,
    blue: 0x8caaee,
};

const CATPPUCCIN_MACCHIATO: CatppuccinPalette = CatppuccinPalette {
    base: 0x24273a,
    mantle: 0x1e2030,
    crust: 0x181926,
    surface0: 0x363a4f,
    surface1: 0x494d64,
    surface2: 0x5b6078,
    text: 0xcad3f5,
    subtext0: 0xa5adcb,
    overlay1: 0x8087a2,
    peach: 0xf5a97f,
    red: 0xed8796,
    green: 0xa6da95,
    yellow: 0xeed49f,
    blue: 0x8aadf4,
};

const CATPPUCCIN_MOCHA: CatppuccinPalette = CatppuccinPalette {
    base: 0x1e1e2e,
    mantle: 0x181825,
    crust: 0x11111b,
    surface0: 0x313244,
    surface1: 0x45475a,
    surface2: 0x585b70,
    text: 0xcdd6f4,
    subtext0: 0xa6adc8,
    overlay1: 0x7f849c,
    peach: 0xfab387,
    red: 0xf38ba8,
    green: 0xa6e3a1,
    yellow: 0xf9e2af,
    blue: 0x89b4fa,
};

fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
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
            ThemeMode::Latte => Self::catppuccin(CATPPUCCIN_LATTE, false),
            ThemeMode::Frappe => Self::catppuccin(CATPPUCCIN_FRAPPE, true),
            ThemeMode::Macchiato => Self::catppuccin(CATPPUCCIN_MACCHIATO, true),
            ThemeMode::Mocha => Self::catppuccin(CATPPUCCIN_MOCHA, true),
            ThemeMode::HighContrast => Self::high_contrast(),
            ThemeMode::Mono => Self::mono(),
        }
    }

    /// Maximum-contrast palette for low-vision terminals.
    pub fn high_contrast() -> Self {
        Self {
            bg: Color::Rgb(0, 0, 0),
            surface: Color::Rgb(0, 0, 0),
            surface_alt: Color::Rgb(0, 0, 0),
            elevated: Color::Rgb(0, 0, 0),
            text: Color::Rgb(255, 255, 255),
            text_dim: Color::Rgb(235, 235, 235),
            muted: Color::Rgb(180, 180, 180),
            accent: Color::Rgb(255, 255, 0),
            accent_soft: Color::Rgb(70, 70, 0),
            on_accent: Color::Rgb(0, 0, 0),
            success: Color::Rgb(0, 255, 128),
            warning: Color::Rgb(255, 255, 0),
            danger: Color::Rgb(255, 80, 80),
            info: Color::Rgb(0, 220, 255),
            border: Color::Rgb(255, 255, 255),
            border_active: Color::Rgb(255, 255, 0),
            selected_bg: Color::Rgb(255, 255, 255),
            selected_fg: Color::Rgb(0, 0, 0),
            badge_bg: Color::Rgb(255, 255, 255),
            badge_fg: Color::Rgb(0, 0, 0),
            code_bg: Color::Rgb(0, 0, 0),
            scroll_track: Color::Rgb(50, 50, 50),
            scroll_thumb: Color::Rgb(255, 255, 255),
        }
    }

    /// Strict monochrome palette, also used for `NO_COLOR`.
    pub fn mono() -> Self {
        Self {
            bg: Color::Rgb(18, 18, 18),
            surface: Color::Rgb(18, 18, 18),
            surface_alt: Color::Rgb(18, 18, 18),
            elevated: Color::Rgb(28, 28, 28),
            text: Color::Rgb(255, 255, 255),
            text_dim: Color::Rgb(220, 220, 220),
            muted: Color::Rgb(150, 150, 150),
            accent: Color::Rgb(255, 255, 255),
            accent_soft: Color::Rgb(60, 60, 60),
            on_accent: Color::Rgb(0, 0, 0),
            success: Color::Rgb(255, 255, 255),
            warning: Color::Rgb(255, 255, 255),
            danger: Color::Rgb(255, 255, 255),
            info: Color::Rgb(255, 255, 255),
            border: Color::Rgb(130, 130, 130),
            border_active: Color::Rgb(255, 255, 255),
            selected_bg: Color::Rgb(255, 255, 255),
            selected_fg: Color::Rgb(0, 0, 0),
            badge_bg: Color::Rgb(255, 255, 255),
            badge_fg: Color::Rgb(0, 0, 0),
            code_bg: Color::Rgb(28, 28, 28),
            scroll_track: Color::Rgb(50, 50, 50),
            scroll_thumb: Color::Rgb(255, 255, 255),
        }
    }

    pub fn background_rgba(mode: ThemeMode) -> [u8; 4] {
        match Self::from_mode(mode).bg {
            Color::Rgb(red, green, blue) => [red, green, blue, 255],
            _ => [0, 0, 0, 255],
        }
    }

    fn catppuccin(palette: CatppuccinPalette, dark: bool) -> Self {
        let base = rgb(palette.base);
        let crust = rgb(palette.crust);
        let text = rgb(palette.text);
        let peach = rgb(palette.peach);
        let surface0 = rgb(palette.surface0);
        let surface1 = rgb(palette.surface1);
        Self {
            bg: base,
            surface: rgb(palette.mantle),
            surface_alt: crust,
            elevated: surface0,
            text,
            text_dim: rgb(palette.subtext0),
            muted: rgb(palette.overlay1),
            accent: peach,
            accent_soft: surface1,
            on_accent: if dark { crust } else { text },
            success: rgb(palette.green),
            warning: rgb(palette.yellow),
            danger: rgb(palette.red),
            info: rgb(palette.blue),
            border: surface1,
            border_active: peach,
            selected_bg: surface1,
            selected_fg: text,
            badge_bg: surface1,
            badge_fg: text,
            code_bg: surface0,
            scroll_track: surface0,
            scroll_thumb: rgb(palette.surface2),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_has_a_distinct_background() {
        let mut seen = Vec::new();
        for mode in ThemeMode::ALL {
            let background = Theme::background_rgba(mode);
            assert!(
                !seen.contains(&background),
                "{mode:?} duplicates a background"
            );
            seen.push(background);
        }
    }

    #[test]
    fn catppuccin_text_is_visible_against_its_base() {
        for mode in [
            ThemeMode::Latte,
            ThemeMode::Frappe,
            ThemeMode::Macchiato,
            ThemeMode::Mocha,
        ] {
            let theme = Theme::from_mode(mode);
            assert_ne!(theme.text, theme.bg, "{mode:?}");
            assert_ne!(theme.accent, theme.bg, "{mode:?}");
        }
    }
}
