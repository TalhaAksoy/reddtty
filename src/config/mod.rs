pub mod presets;
pub mod settings;

pub use settings::{
    HtmlSettings, ImageProtocolSetting, ProviderKind, ProviderSettings, Settings, ThemeMode,
    validate_html_base,
};
