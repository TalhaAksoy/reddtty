use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::config::{ImageProtocolSetting, ProviderKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ThemeArg {
    Dark,
    Light,
    #[value(alias = "catppuccin-latte")]
    Latte,
    #[value(alias = "catppuccin-frappe")]
    Frappe,
    #[value(alias = "catppuccin-macchiato")]
    Macchiato,
    #[value(alias = "catppuccin-mocha")]
    Mocha,
    #[value(alias = "high-contrast")]
    HighContrast,
    #[value(alias = "mono", alias = "monochrome")]
    Mono,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ProtocolArg {
    Auto,
    Kitty,
    Iterm2,
    Sixel,
    Halfblocks,
    Disabled,
}

impl From<ProtocolArg> for ImageProtocolSetting {
    fn from(value: ProtocolArg) -> Self {
        match value {
            ProtocolArg::Auto => Self::Auto,
            ProtocolArg::Kitty => Self::Kitty,
            ProtocolArg::Iterm2 => Self::Iterm2,
            ProtocolArg::Sixel => Self::Sixel,
            ProtocolArg::Halfblocks => Self::Halfblocks,
            ProtocolArg::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ProviderArg {
    Auto,
    Demo,
    Reddit,
    OldReddit,
    Redlib,
}

impl From<ProviderArg> for ProviderKind {
    fn from(value: ProviderArg) -> Self {
        match value {
            ProviderArg::Auto => Self::Auto,
            ProviderArg::Demo => Self::Demo,
            ProviderArg::Reddit => Self::Reddit,
            ProviderArg::OldReddit => Self::OldReddit,
            ProviderArg::Redlib => Self::Redlib,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "reddtty",
    version,
    about = "A modern, image-capable Reddit reader for the terminal",
    long_about = "ReddTTY is a keyboard-first terminal Reddit reader. It runs on synthetic demo data unless Reddit OAuth credentials are configured. It is not affiliated with Reddit.",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Use the embedded demo data (works offline, no Reddit credentials).
    #[arg(long)]
    pub demo: bool,

    /// Select the content source for this session.
    #[arg(long, value_enum, value_name = "PROVIDER")]
    pub provider: Option<ProviderArg>,

    /// Base URL of the Redlib instance to use (with --provider redlib).
    #[arg(long, value_name = "URL")]
    pub redlib_instance: Option<String>,

    /// Open a subreddit directly (accepts rust, r/rust or a reddit.com URL).
    #[arg(short = 'r', long, value_name = "NAME")]
    pub subreddit: Option<String>,

    /// Initial feed sort: best, hot, new, top, rising or controversial.
    #[arg(short = 's', long, value_name = "SORT")]
    pub sort: Option<String>,

    /// Start with dark, light, or a Catppuccin theme (latte, frappe, macchiato, mocha).
    #[arg(long, value_enum, value_name = "THEME")]
    pub theme: Option<ThemeArg>,

    /// Disable all terminal image rendering for this session.
    #[arg(long)]
    pub no_images: bool,

    /// Force the monochrome theme for this session (also honours NO_COLOR).
    #[arg(long)]
    pub no_color: bool,

    /// Override the terminal image protocol for this session.
    #[arg(long, value_enum, value_name = "PROTOCOL")]
    pub images_protocol: Option<ProtocolArg>,

    /// Print terminal, image and configuration diagnostics (no secrets).
    #[arg(long)]
    pub diagnostics: bool,

    /// Use a specific configuration file instead of the default location.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_flags() {
        let cli = Cli::try_parse_from([
            "reddtty",
            "--demo",
            "--subreddit",
            "rust",
            "--sort",
            "new",
            "--theme",
            "light",
            "--no-images",
        ])
        .unwrap();
        assert!(cli.demo);
        assert_eq!(cli.subreddit.as_deref(), Some("rust"));
        assert_eq!(cli.sort.as_deref(), Some("new"));
        assert_eq!(cli.theme, Some(ThemeArg::Light));
        assert!(cli.no_images);
        assert!(cli.provider.is_none());
    }

    #[test]
    fn parses_no_color() {
        let cli = Cli::try_parse_from(["reddtty", "--no-color"]).unwrap();
        assert!(cli.no_color);
    }

    #[test]
    fn parses_catppuccin_theme() {
        let cli = Cli::try_parse_from(["reddtty", "--theme", "mocha"]).unwrap();
        assert_eq!(cli.theme, Some(ThemeArg::Mocha));
    }

    #[test]
    fn parses_provider_flags() {
        let cli = Cli::try_parse_from([
            "reddtty",
            "--provider",
            "old-reddit",
            "--redlib-instance",
            "https://redlib.example",
        ])
        .unwrap();
        assert_eq!(cli.provider, Some(ProviderArg::OldReddit));
        assert_eq!(
            cli.redlib_instance.as_deref(),
            Some("https://redlib.example")
        );
        assert_eq!(
            ProviderKind::from(ProviderArg::Redlib),
            ProviderKind::Redlib
        );
    }

    #[test]
    fn protocol_mapping() {
        assert_eq!(
            ImageProtocolSetting::from(ProtocolArg::Iterm2),
            ImageProtocolSetting::Iterm2
        );
        assert_eq!(
            ImageProtocolSetting::from(ProtocolArg::Disabled),
            ImageProtocolSetting::Disabled
        );
    }
}
