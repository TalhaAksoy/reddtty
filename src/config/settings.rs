use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::text::normalize_subreddit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    #[serde(alias = "catppuccin-latte", alias = "catppuccin_latte")]
    Latte,
    #[serde(alias = "catppuccin-frappe", alias = "catppuccin_frappe")]
    Frappe,
    #[serde(alias = "catppuccin-macchiato", alias = "catppuccin_macchiato")]
    Macchiato,
    #[serde(alias = "catppuccin-mocha", alias = "catppuccin_mocha")]
    Mocha,
}

impl ThemeMode {
    pub const ALL: [Self; 6] = [
        Self::Dark,
        Self::Light,
        Self::Latte,
        Self::Frappe,
        Self::Macchiato,
        Self::Mocha,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Latte => "Catppuccin Latte",
            Self::Frappe => "Catppuccin Frappe",
            Self::Macchiato => "Catppuccin Macchiato",
            Self::Mocha => "Catppuccin Mocha",
        }
    }

    /// Cycle themes with `T`: dark, light, Latte, Frappé, Macchiato, Mocha.
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|mode| *mode == self)
            .unwrap_or_default();
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn toggle(self) -> Self {
        self.next()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageProtocolSetting {
    #[default]
    Auto,
    Kitty,
    Iterm2,
    Sixel,
    Halfblocks,
    Disabled,
}

impl ImageProtocolSetting {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Kitty => "Kitty",
            Self::Iterm2 => "iTerm2",
            Self::Sixel => "Sixel",
            Self::Halfblocks => "Halfblocks",
            Self::Disabled => "Disabled",
        }
    }

    pub const ALL: [Self; 6] = [
        Self::Auto,
        Self::Kitty,
        Self::Iterm2,
        Self::Sixel,
        Self::Halfblocks,
        Self::Disabled,
    ];

    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "kitty" => Some(Self::Kitty),
            "iterm" | "iterm2" => Some(Self::Iterm2),
            "sixel" => Some(Self::Sixel),
            "halfblocks" | "half-blocks" | "unicode" => Some(Self::Halfblocks),
            "disabled" | "none" | "off" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    pub theme: ThemeMode,
    pub default_feed: String,
    pub default_sort: String,
    pub show_sidebar: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            default_feed: "home".into(),
            default_sort: "best".into(),
            show_sidebar: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSettings {
    pub enabled: bool,
    pub protocol: ImageProtocolSetting,
    pub preview_in_feed: bool,
    pub max_cache_mb: u64,
    pub max_download_mb: u64,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            protocol: ImageProtocolSetting::Auto,
            preview_in_feed: true,
            max_cache_mb: 128,
            max_download_mb: 10,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentSettings {
    pub show_nsfw: bool,
    pub hide_spoilers: bool,
    pub default_post_limit: u32,
}

impl Default for ContentSettings {
    fn default() -> Self {
        Self {
            show_nsfw: false,
            hide_spoilers: true,
            default_post_limit: 25,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub mouse_enabled: bool,
    pub show_right_panel: bool,
    pub relative_time: bool,
    pub sidebar_width: u16,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
            show_right_panel: true,
            relative_time: true,
            sidebar_width: 24,
        }
    }
}

/// Reddit credentials. These values may also be supplied through environment
/// variables; environment variables always win and are never written to disk by
/// the app.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RedditSettings {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub username: String,
    pub user_agent: String,
    pub use_environment: bool,
}

/// Which content source ReddTTY uses.
///
/// `Auto` preserves the historical behaviour: use Reddit OAuth when
/// credentials are configured, otherwise fall back to the embedded demo data.
/// Explicitly selecting a provider never silently falls back to demo data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    #[default]
    Auto,
    Demo,
    Reddit,
    OldReddit,
    Redlib,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Demo => "demo",
            Self::Reddit => "reddit",
            Self::OldReddit => "old-reddit",
            Self::Redlib => "redlib",
        }
    }

    pub fn is_html(self) -> bool {
        matches!(self, Self::OldReddit | Self::Redlib)
    }

    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "auto" => Some(Self::Auto),
            "demo" | "offline" => Some(Self::Demo),
            "reddit" | "oauth" | "api" => Some(Self::Reddit),
            "old-reddit" | "old" | "oldreddit" => Some(Self::OldReddit),
            "redlib" => Some(Self::Redlib),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmlSettings {
    /// Base URL of the user's chosen Redlib instance. Never picked
    /// automatically; an unavailable instance is reported, not rotated.
    pub redlib_instance: String,
    /// Enables live HTTP requests for the HTML providers. This is an explicit
    /// acknowledgement that the user has checked the source's terms and
    /// permissions. It is never enabled by default.
    pub allow_network: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProviderSettings {
    #[serde(rename = "type")]
    pub kind: ProviderKind,
    pub html: HtmlSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub general: GeneralSettings,
    pub images: ImageSettings,
    pub content: ContentSettings,
    pub ui: UiSettings,
    pub reddit: RedditSettings,
    pub provider: ProviderSettings,
    pub favorites: Vec<String>,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            images: ImageSettings::default(),
            content: ContentSettings::default(),
            ui: UiSettings::default(),
            reddit: RedditSettings {
                user_agent: default_user_agent(),
                use_environment: true,
                ..RedditSettings::default()
            },
            provider: ProviderSettings::default(),
            favorites: Vec::new(),
            recent: Vec::new(),
        }
    }
}

fn migrate_config_file(
    new_path: &Path,
    legacy_dirs: &[PathBuf],
) -> Result<Option<PathBuf>, String> {
    if new_path.is_file() {
        return Ok(None);
    }
    let Some(old_path) = legacy_dirs
        .iter()
        .map(|dir| dir.join("config.toml"))
        .find(|path| path.is_file())
    else {
        return Ok(None);
    };
    let parent = new_path
        .parent()
        .ok_or_else(|| "the new config path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    if new_path.is_file() {
        return Ok(None);
    }
    let tmp = parent.join(format!(".config.toml.migrating-{}", std::process::id()));
    let copy_result = fs::copy(&old_path, &tmp);
    if let Err(error) = copy_result {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "could not copy {} to {}: {error}",
            old_path.display(),
            new_path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    if new_path.is_file() {
        let _ = fs::remove_file(&tmp);
        return Ok(None);
    }
    normalize_migrated_user_agent(&tmp);
    if let Err(error) = fs::rename(&tmp, new_path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("could not install {}: {error}", new_path.display()));
    }
    Ok(Some(old_path))
}

/// Rewrite only the `user_agent` line in a copied legacy config. The old file
/// remains untouched; this keeps the new config free of the legacy brand while
/// preserving every other value byte-for-byte.
fn normalize_migrated_user_agent(path: &Path) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let normalized: String = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("user_agent") {
                normalize_legacy_user_agent(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if normalized != text.trim_end_matches('\n') {
        let mut output = normalized;
        output.push('\n');
        let _ = fs::write(path, output);
    }
}

fn normalize_legacy_user_agent(value: &str) -> String {
    value
        .replace("THREADLINE", "REDDTTY")
        .replace("Threadline", "ReddTTY")
        .replace("threadline", "reddtty")
}

pub fn default_user_agent() -> String {
    format!(
        "reddtty/{} (terminal Reddit reader; +https://example.invalid/reddtty)",
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedditCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: Option<String>,
    pub username: Option<String>,
    pub user_agent: String,
}

const APP_DIR: &str = "reddtty";
const LEGACY_APP_DIR: &str = "threadline";

impl Settings {
    fn config_root() -> PathBuf {
        #[cfg(windows)]
        {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        }
        #[cfg(not(windows))]
        {
            if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
            {
                return xdg;
            }
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        }
    }

    pub fn config_dir() -> PathBuf {
        // Use the XDG-style location on macOS as well as Linux so the same
        // `~/.config/reddtty/config.toml` path works across Unix systems.
        // Windows keeps its conventional roaming AppData location.
        Self::config_root().join(APP_DIR)
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(APP_DIR)
    }

    /// Legacy directories from before the ReddTTY rename. They are only read
    /// for a one-time settings migration; they are never used after the new
    /// directory exists.
    pub fn legacy_config_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![Self::config_root().join(LEGACY_APP_DIR)];
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                dirs.push(
                    home.join("Library")
                        .join("Application Support")
                        .join(LEGACY_APP_DIR),
                );
            }
        }
        dirs.sort();
        dirs.dedup();
        dirs
    }

    /// Resolve the default config path and perform the one-time migration from
    /// `threadline` if needed. A custom `--config PATH` must not trigger this.
    pub fn migrate_default_config() -> (PathBuf, Vec<String>) {
        let new_path = Self::config_path();
        let legacy = Self::legacy_config_dirs();
        let mut warnings = Vec::new();
        match migrate_config_file(&new_path, &legacy) {
            Ok(Some(from)) => warnings.push(format!(
                "Migrated settings from {} to {}. The old file was left in place.",
                from.display(),
                new_path.display()
            )),
            Ok(None) => {}
            Err(error) => warnings.push(format!(
                "Could not migrate settings: {error}. Existing files were left untouched."
            )),
        }
        (new_path, warnings)
    }

    /// Load settings, falling back to safe defaults for a missing or malformed
    /// file. Warnings are surfaced in the UI instead of crashing the app.
    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), warnings);
            }
            Err(err) => {
                warnings.push(format!(
                    "Could not read {}: {err}. Using default settings.",
                    path.display()
                ));
                return (Self::default(), warnings);
            }
        };

        match toml::from_str::<Settings>(&text) {
            Ok(mut settings) => {
                warnings.extend(settings.validate());
                (settings, warnings)
            }
            Err(err) => {
                warnings.push(format!(
                    "Invalid config ({}): {} Using default settings.",
                    path.display(),
                    first_line(&err.to_string())
                ));
                (Self::default(), warnings)
            }
        }
    }

    /// Validate and clamp user-editable values.
    pub fn validate(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        if !(5..=100).contains(&self.content.default_post_limit) {
            warnings.push("default_post_limit clamped to 5–100.".into());
            self.content.default_post_limit = self.content.default_post_limit.clamp(5, 100);
        }
        if !(8..=2048).contains(&self.images.max_cache_mb) {
            warnings.push("max_cache_mb clamped to 8–2048.".into());
            self.images.max_cache_mb = self.images.max_cache_mb.clamp(8, 2048);
        }
        if !(1..=64).contains(&self.images.max_download_mb) {
            warnings.push("max_download_mb clamped to 1–64.".into());
            self.images.max_download_mb = self.images.max_download_mb.clamp(1, 64);
        }
        if !(16..=48).contains(&self.ui.sidebar_width) {
            warnings.push("sidebar_width clamped to 16–48.".into());
            self.ui.sidebar_width = self.ui.sidebar_width.clamp(16, 48);
        }
        let feed = self.general.default_feed.trim().to_ascii_lowercase();
        if !feed.is_empty()
            && feed != "home"
            && feed != "popular"
            && normalize_subreddit(&feed).is_none()
        {
            warnings.push("default_feed was invalid; using Home.".into());
            self.general.default_feed = "home".into();
        }
        if crate::models::FeedSort::from_str_loose(&self.general.default_sort).is_none() {
            warnings.push("default_sort was invalid; using best.".into());
            self.general.default_sort = "best".into();
        }

        let normalized_user_agent = normalize_legacy_user_agent(&self.reddit.user_agent);
        if normalized_user_agent != self.reddit.user_agent {
            self.reddit.user_agent = normalized_user_agent;
            warnings.push("user_agent was updated from the legacy product name.".into());
        }
        if self.reddit.user_agent.trim().is_empty() {
            warnings.push("Empty user_agent replaced with the default.".into());
            self.reddit.user_agent = default_user_agent();
        }
        if !self.reddit.user_agent.contains("reddtty") {
            warnings.push("user_agent should identify ReddTTY and a contact URL/username.".into());
        }

        let instance = self.provider.html.redlib_instance.trim().to_string();
        if !instance.is_empty() {
            match validate_html_base(&instance) {
                Ok(normalised) => {
                    if normalised != self.provider.html.redlib_instance {
                        self.provider.html.redlib_instance = normalised;
                    }
                }
                Err(reason) => {
                    warnings.push(format!("redlib_instance was rejected: {reason}"));
                    self.provider.html.redlib_instance.clear();
                }
            }
        }
        if self.provider.kind == ProviderKind::Redlib
            && self.provider.html.redlib_instance.is_empty()
        {
            warnings
                .push("provider.type = \"redlib\" requires provider.html.redlib_instance.".into());
        }
        if self.provider.kind.is_html() && !self.provider.html.allow_network {
            warnings.push(
                "HTML provider selected but provider.html.allow_network is false; live requests are disabled."
                    .into(),
            );
        }

        let mut favorites = Vec::new();
        for favorite in &self.favorites {
            if let Some(name) = normalize_subreddit(favorite)
                && !favorites.contains(&name)
            {
                favorites.push(name);
            }
        }
        if favorites != self.favorites {
            warnings.push("Favorites were normalised and de-duplicated.".into());
            self.favorites = favorites;
        }

        let mut recent = Vec::new();
        for name in &self.recent {
            if let Some(name) = normalize_subreddit(name)
                && !recent.contains(&name)
            {
                recent.push(name);
            }
        }
        recent.truncate(12);
        self.recent = recent;

        warnings
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create config directory: {e}"))?;
        }
        let text =
            toml::to_string_pretty(self).map_err(|e| format!("Could not serialise config: {e}"))?;
        let tmp = path.with_extension("toml.tmp");
        {
            let mut file =
                fs::File::create(&tmp).map_err(|e| format!("Could not write config: {e}"))?;
            file.write_all(text.as_bytes())
                .map_err(|e| format!("Could not write config: {e}"))?;
            file.flush()
                .map_err(|e| format!("Could not flush config: {e}"))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        #[cfg(windows)]
        {
            // Windows cannot rename over an existing file.
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp, path).map_err(|e| format!("Could not replace config: {e}"))?;
        Ok(())
    }

    pub fn add_recent(&mut self, name: &str) {
        if let Some(name) = normalize_subreddit(name) {
            self.recent.retain(|n| n != &name);
            self.recent.insert(0, name);
            self.recent.truncate(12);
        }
    }

    pub fn is_favorite(&self, name: &str) -> bool {
        self.favorites.iter().any(|f| f.eq_ignore_ascii_case(name))
    }

    /// Toggle favorite status and return the new state.
    pub fn toggle_favorite(&mut self, name: &str) -> bool {
        if let Some(name) = normalize_subreddit(name) {
            if self.is_favorite(&name) {
                self.favorites.retain(|f| !f.eq_ignore_ascii_case(&name));
                false
            } else {
                self.favorites.push(name);
                true
            }
        } else {
            false
        }
    }

    pub fn theme(&self) -> ThemeMode {
        self.general.theme
    }

    /// Resolve Reddit credentials, preferring environment variables.
    ///
    /// `REDDTTY_*` wins over the legacy `THREADLINE_*` names, which are kept
    /// only for migration compatibility. Values are never logged.
    pub fn reddit_credentials(&self) -> Option<RedditCredentials> {
        self.reddit_credentials_with(|key| std::env::var(key).ok())
    }

    fn env_credential<F>(&self, env: &F, key: &str) -> Option<String>
    where
        F: Fn(&str) -> Option<String>,
    {
        if !self.reddit.use_environment {
            return None;
        }
        let clean = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
        clean(env(&format!("REDDTTY_{key}"))).or_else(|| clean(env(&format!("THREADLINE_{key}"))))
    }

    fn reddit_credentials_with<F>(&self, env: F) -> Option<RedditCredentials>
    where
        F: Fn(&str) -> Option<String>,
    {
        let client_id = self
            .env_credential(&env, "REDDIT_CLIENT_ID")
            .unwrap_or_else(|| self.reddit.client_id.clone())
            .trim()
            .to_string();
        let client_secret = self
            .env_credential(&env, "REDDIT_CLIENT_SECRET")
            .unwrap_or_else(|| self.reddit.client_secret.clone())
            .trim()
            .to_string();
        let refresh_token = self
            .env_credential(&env, "REDDIT_REFRESH_TOKEN")
            .or_else(|| {
                let token = self.reddit.refresh_token.trim();
                (!token.is_empty()).then(|| token.to_string())
            });
        let username = self.env_credential(&env, "REDDIT_USERNAME").or_else(|| {
            let name = self.reddit.username.trim();
            (!name.is_empty()).then(|| name.to_string())
        });
        let user_agent = normalize_legacy_user_agent(
            &self
                .env_credential(&env, "REDDIT_USER_AGENT")
                .unwrap_or_else(|| self.reddit.user_agent.clone()),
        );

        if client_id.is_empty() || client_secret.is_empty() {
            None
        } else {
            Some(RedditCredentials {
                client_id,
                client_secret,
                refresh_token,
                username,
                user_agent,
            })
        }
    }

    /// Redacted diagnostics: never print secrets.
    pub fn redacted_summary(&self) -> String {
        let creds = self.reddit_credentials();
        format!(
            "theme={} images={} protocol={} sidebar={} right_panel={} limit={} provider={} reddit_configured={} refresh_token={} html_network={} redlib_instance={} config={}",
            self.general.theme.label(),
            self.images.enabled,
            self.images.protocol.label(),
            self.general.show_sidebar,
            self.ui.show_right_panel,
            self.content.default_post_limit,
            self.provider.kind.label(),
            creds.is_some(),
            creds.as_ref().is_some_and(|c| c.refresh_token.is_some()),
            self.provider.html.allow_network,
            self.provider.html.redlib_instance,
            Self::config_path().display()
        )
    }
}

/// Validate and normalise a user-supplied HTML source URL.
///
/// Live sources must use HTTPS (loopback HTTP is allowed for local fixtures and
/// self-hosted instances). Credentials, query strings and fragments are
/// rejected because they are not part of a base URL and can leak secrets.
pub fn validate_html_base(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw.trim()).map_err(|e| format!("invalid URL: {e}"))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err("only http(s) URLs are supported".into());
    }
    if parsed.host_str().is_none() {
        return Err("the URL must include a host".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("credentials must not be embedded in the URL".into());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("the base URL must not contain a query or fragment".into());
    }
    if !parsed.path().is_empty() && parsed.path() != "/" {
        return Err("the instance URL must point at the site root, not a subpath".into());
    }
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let loopback =
        host == "localhost" || host == "127.0.0.1" || host == "::1" || host.ends_with(".localhost");
    if parsed.scheme() == "http" && !loopback {
        return Err(
            "live HTML sources must use https; plain http is only allowed for loopback tests"
                .into(),
        );
    }
    let mut normalised = parsed.to_string();
    while normalised.ends_with('/') {
        normalised.pop();
    }
    Ok(normalised)
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.general.theme, ThemeMode::Dark);
        assert_eq!(s.images.max_cache_mb, 128);
        assert!(!s.content.show_nsfw);
        assert!(s.content.hide_spoilers);
        assert!(s.images.enabled);
        assert!(s.favorites.is_empty());
    }

    #[test]
    fn partial_config_uses_defaults() {
        let text = r#"
[general]
theme = "light"

[images]
max_cache_mb = 32
"#;
        let s: Settings = toml::from_str(text).unwrap();
        assert_eq!(s.general.theme, ThemeMode::Light);
        assert_eq!(s.images.max_cache_mb, 32);
        assert!(s.images.enabled); // default
        assert!(!s.content.show_nsfw);
    }

    #[test]
    fn invalid_config_falls_back_with_warning() {
        let dir = std::env::temp_dir().join(format!("reddtty-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("broken.toml");
        std::fs::write(&path, "this is not toml [[[").unwrap();
        let (settings, warnings) = Settings::load(&path);
        assert_eq!(settings.general.theme, ThemeMode::Dark);
        assert!(!warnings.is_empty());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn favorites_toggle_and_normalise() {
        let mut s = Settings::default();
        assert!(!s.is_favorite("rust"));
        assert!(s.toggle_favorite("r/rust"));
        assert!(s.is_favorite("rust"));
        assert!(!s.toggle_favorite("rust"));
        assert!(!s.is_favorite("rust"));
        assert!(s.toggle_favorite("r/Weird_Name"));
        assert!(s.is_favorite("weird_name"));
    }

    #[test]
    fn recents_are_moved_to_front() {
        let mut s = Settings::default();
        s.add_recent("rust");
        s.add_recent("linux");
        s.add_recent("rust");
        assert_eq!(s.recent, vec!["rust", "linux"]);
    }

    #[test]
    fn validate_clamps_values() {
        let mut s = Settings::default();
        s.content.default_post_limit = 1000;
        s.images.max_cache_mb = 1;
        s.ui.sidebar_width = 100;
        s.reddit.user_agent.clear();
        let warnings = s.validate();
        assert_eq!(s.content.default_post_limit, 100);
        assert_eq!(s.images.max_cache_mb, 8);
        assert_eq!(s.ui.sidebar_width, 48);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn invalid_default_feed_and_sort_are_repaired() {
        let mut s = Settings::default();
        s.general.default_feed = "not a valid feed!!!".into();
        s.general.default_sort = "sideways".into();
        let warnings = s.validate();
        assert_eq!(s.general.default_feed, "home");
        assert_eq!(s.general.default_sort, "best");
        assert!(warnings.len() >= 2);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("reddtty-save-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        let mut s = Settings::default();
        s.general.theme = ThemeMode::Light;
        s.images.protocol = ImageProtocolSetting::Kitty;
        s.save(&path).unwrap();
        let (loaded, warnings) = Settings::load(&path);
        assert!(warnings.is_empty());
        assert_eq!(loaded.general.theme, ThemeMode::Light);
        assert_eq!(loaded.images.protocol, ImageProtocolSetting::Kitty);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn migration_copies_legacy_settings_and_does_not_overwrite() {
        let root = test_temp_dir("migrate-copy");
        let old_dir = root.join("old");
        let new_dir = root.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        let old_path = old_dir.join("config.toml");
        std::fs::write(
            &old_path,
            r#"
favorites = ["rust", "linux"]
[reddit]
user_agent = "threadline/0.1 (contact: x)"
[general]
theme = "light"
"#,
        )
        .unwrap();
        let new_path = new_dir.join("config.toml");

        let migrated = migrate_config_file(&new_path, std::slice::from_ref(&old_dir)).unwrap();
        assert_eq!(migrated.as_deref(), Some(old_path.as_path()));
        assert!(new_path.is_file());
        assert!(old_path.is_file(), "legacy config must not be deleted");
        let (loaded, warnings) = Settings::load(&new_path);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.general.theme, ThemeMode::Light);
        assert_eq!(loaded.favorites, vec!["rust", "linux"]);
        let migrated_text = std::fs::read_to_string(&new_path).unwrap();
        assert!(migrated_text.contains("user_agent = \"reddtty/0.1"));
        assert!(!migrated_text.contains("threadline/0.1"));

        let second = migrate_config_file(&new_path, &[old_dir]).unwrap();
        assert_eq!(second, None, "migration must not run twice");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_never_overwrites_an_existing_new_config() {
        let root = test_temp_dir("migrate-existing");
        let old_dir = root.join("old");
        let new_dir = root.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::create_dir_all(&new_dir).unwrap();
        std::fs::write(
            old_dir.join("config.toml"),
            "[general]\ntheme = \"light\"\n",
        )
        .unwrap();
        std::fs::write(new_dir.join("config.toml"), "[general]\ntheme = \"dark\"\n").unwrap();
        let new_path = new_dir.join("config.toml");
        assert_eq!(
            migrate_config_file(&new_path, std::slice::from_ref(&old_dir)).unwrap(),
            None
        );
        let (loaded, _) = Settings::load(&new_path);
        assert_eq!(loaded.general.theme, ThemeMode::Dark);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn broken_legacy_config_is_copied_and_load_reports_the_parse_error() {
        let root = test_temp_dir("migrate-broken");
        let old_dir = root.join("old");
        let new_dir = root.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("config.toml"), "this is not toml [[[").unwrap();
        let new_path = new_dir.join("config.toml");
        assert!(
            migrate_config_file(&new_path, std::slice::from_ref(&old_dir))
                .unwrap()
                .is_some()
        );
        let (_loaded, warnings) = Settings::load(&new_path);
        assert!(!warnings.is_empty());
        assert!(old_dir.join("config.toml").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_legacy_bytes_are_copied_and_load_warns_without_deleting() {
        let root = test_temp_dir("migrate-unreadable");
        let old_dir = root.join("old");
        let new_dir = root.join("new");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("config.toml"), [0xff, 0xfe, 0x00]).unwrap();
        let new_path = new_dir.join("config.toml");
        assert!(
            migrate_config_file(&new_path, std::slice::from_ref(&old_dir))
                .unwrap()
                .is_some()
        );
        let (_loaded, warnings) = Settings::load(&new_path);
        assert!(!warnings.is_empty());
        assert!(old_dir.join("config.toml").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migration_reports_destination_creation_failure_without_deleting_legacy() {
        let root = test_temp_dir("migrate-dest-fail");
        let old_dir = root.join("old");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(
            old_dir.join("config.toml"),
            "[general]\ntheme = \"light\"\n",
        )
        .unwrap();
        let parent_file = root.join("not-a-directory");
        std::fs::write(&parent_file, "x").unwrap();
        let new_path = parent_file.join("config.toml");
        let error = migrate_config_file(&new_path, std::slice::from_ref(&old_dir)).unwrap_err();
        assert!(error.contains("could not create"), "{error}");
        assert!(old_dir.join("config.toml").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn new_environment_names_win_and_legacy_names_still_work() {
        let settings = Settings::default();
        let credentials = settings
            .reddit_credentials_with(|key| match key {
                "REDDTTY_REDDIT_CLIENT_ID" => Some("new-id".into()),
                "REDDTTY_REDDIT_CLIENT_SECRET" => Some("new-secret".into()),
                "THREADLINE_REDDIT_CLIENT_ID" => Some("old-id".into()),
                "THREADLINE_REDDIT_CLIENT_SECRET" => Some("old-secret".into()),
                "THREADLINE_REDDIT_USER_AGENT" => Some("threadline/old (contact: x)".into()),
                _ => None,
            })
            .unwrap();
        assert_eq!(credentials.client_id, "new-id");
        assert_eq!(credentials.client_secret, "new-secret");
        assert!(credentials.user_agent.starts_with("reddtty/"));

        let legacy = settings
            .reddit_credentials_with(|key| match key {
                "THREADLINE_REDDIT_CLIENT_ID" => Some("old-id".into()),
                "THREADLINE_REDDIT_CLIENT_SECRET" => Some("old-secret".into()),
                _ => None,
            })
            .unwrap();
        assert_eq!(legacy.client_id, "old-id");
        assert_eq!(legacy.client_secret, "old-secret");
    }

    #[test]
    fn config_fallback_is_used_when_no_environment_is_present() {
        let mut settings = Settings::default();
        settings.reddit.client_id = "config-id".into();
        settings.reddit.client_secret = "config-secret".into();
        settings.reddit.user_agent = "threadline/0.1 (contact: x)".into();
        let credentials = settings.reddit_credentials_with(|_| None).unwrap();
        assert_eq!(credentials.client_id, "config-id");
        assert_eq!(credentials.client_secret, "config-secret");
        assert_eq!(credentials.user_agent, "reddtty/0.1 (contact: x)");
    }

    #[cfg(not(windows))]
    #[test]
    fn default_config_dir_is_xdg_style_on_macos_and_linux() {
        if std::env::var_os("XDG_CONFIG_HOME").is_none() {
            assert!(Settings::config_dir().ends_with(".config/reddtty"));
        }
        assert!(Settings::config_path().ends_with("reddtty/config.toml"));
    }

    fn test_temp_dir(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("reddtty-{label}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn theme_cycles_through_light_and_catppuccin() {
        assert_eq!(ThemeMode::Dark.toggle(), ThemeMode::Light);
        assert_eq!(ThemeMode::Light.next(), ThemeMode::Latte);
        assert_eq!(ThemeMode::Mocha.next(), ThemeMode::Dark);
        assert_eq!(ThemeMode::Latte.label(), "Catppuccin Latte");
        #[derive(serde::Deserialize)]
        struct Holder {
            theme: ThemeMode,
        }
        let holder: Holder = toml::from_str("theme = \"mocha\"").unwrap();
        assert_eq!(holder.theme, ThemeMode::Mocha);
    }

    #[test]
    fn provider_kind_parsing_and_defaults() {
        assert_eq!(ProviderKind::default(), ProviderKind::Auto);
        assert_eq!(
            ProviderKind::from_str_loose("OLD_reddit"),
            Some(ProviderKind::OldReddit)
        );
        assert_eq!(
            ProviderKind::from_str_loose("redlib"),
            Some(ProviderKind::Redlib)
        );
        assert!(!Settings::default().provider.html.allow_network);
    }

    #[test]
    fn provider_toml_example_parses() {
        let text = r#"
[provider]
type = "old-reddit"

[provider.html]
redlib_instance = "https://redlib.example"
allow_network = true
"#;
        let settings: Settings = toml::from_str(text).unwrap();
        assert_eq!(settings.provider.kind, ProviderKind::OldReddit);
        assert!(settings.provider.html.allow_network);
        assert_eq!(
            settings.provider.html.redlib_instance,
            "https://redlib.example"
        );
    }

    #[test]
    fn html_base_validation_rejects_unsafe_urls() {
        assert_eq!(
            validate_html_base("https://redlib.example/").unwrap(),
            "https://redlib.example"
        );
        assert!(validate_html_base("http://redlib.example").is_err());
        assert!(validate_html_base("https://user:pass@redlib.example").is_err());
        assert!(validate_html_base("https://redlib.example/?q=x").is_err());
        assert!(validate_html_base("file:///etc/passwd").is_err());
        assert_eq!(
            validate_html_base("http://127.0.0.1:8080/").unwrap(),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn credentials_can_come_from_config() {
        // The test only checks the config-file path; env vars are process-wide
        // and might be set on the developer machine.
        let mut s = Settings::default();
        s.reddit.client_id = "id".into();
        s.reddit.client_secret = "secret".into();
        s.reddit.use_environment = false;
        let creds = s.reddit_credentials().unwrap();
        assert_eq!(creds.client_id, "id");
        assert!(creds.refresh_token.is_none());
    }

    #[test]
    fn protocol_parsing_aliases() {
        assert_eq!(
            ImageProtocolSetting::from_str_loose("iTerm2"),
            Some(ImageProtocolSetting::Iterm2)
        );
        assert_eq!(
            ImageProtocolSetting::from_str_loose("half-blocks"),
            Some(ImageProtocolSetting::Halfblocks)
        );
        assert_eq!(
            ImageProtocolSetting::from_str_loose("off"),
            Some(ImageProtocolSetting::Disabled)
        );
    }
}
