pub mod demo;
pub mod html;
pub mod reddit;
pub mod traits;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::config::{ProviderKind, Settings};
use crate::error::ProviderError;
use crate::models::{Comment, CommentSort, FeedPage, FeedRequest, Post, Subreddit};
use demo::DemoProvider;
use html::{HtmlClientConfig, OldRedditProvider, RedlibProvider, policy_note};
use reddit::{RedditConfig, RedditProvider};

pub use traits::{ContentProvider, ProviderCapabilities, ProviderStatus};

/// Result of provider construction. Demo/live ambiguity is always explained.
pub struct ProviderSetup {
    pub provider: Arc<dyn ContentProvider>,
    pub notices: Vec<String>,
    pub demo: bool,
}

/// A provider that cannot start (missing settings, disabled network, invalid
/// instance). It reports the reason instead of silently switching to demo data.
struct FailingProvider {
    name: &'static str,
    label: &'static str,
    error: ProviderError,
    html: bool,
}

#[async_trait]
impl ContentProvider for FailingProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn short_name(&self) -> &'static str {
        self.label
    }

    fn is_demo(&self) -> bool {
        false
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    fn supported_sorts(&self, spec: &crate::models::FeedSpec) -> Vec<crate::models::FeedSort> {
        if self.html {
            html::sort_options(spec)
        } else {
            spec.sort_options()
        }
    }

    async fn get_feed(&self, _request: FeedRequest) -> Result<FeedPage, ProviderError> {
        Err(self.error.clone())
    }

    async fn get_post(&self, _post_id: &str) -> Result<Post, ProviderError> {
        Err(self.error.clone())
    }

    async fn get_comments(
        &self,
        _post_id: &str,
        _sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        Err(self.error.clone())
    }

    async fn get_subreddit(&self, _name: &str) -> Result<Subreddit, ProviderError> {
        Err(self.error.clone())
    }
}

fn failing(
    name: &'static str,
    label: &'static str,
    error: ProviderError,
    notices: Vec<String>,
    html: bool,
) -> ProviderSetup {
    notices
        .iter()
        .for_each(|notice| tracing::warn!(%notice, "provider setup"));
    ProviderSetup {
        provider: Arc::new(FailingProvider {
            name,
            label,
            error,
            html,
        }),
        notices,
        demo: false,
    }
}

fn demo_provider(notices: Vec<String>) -> ProviderSetup {
    let cache_dir = Settings::cache_dir();
    ProviderSetup {
        provider: Arc::new(DemoProvider::new(&cache_dir)),
        notices,
        demo: true,
    }
}

fn reddit_provider(settings: &Settings, notices: Vec<String>) -> ProviderSetup {
    let Some(creds) = settings.reddit_credentials() else {
        return failing(
            "Reddit OAuth",
            "REDDIT",
            ProviderError::AuthRequired(
                "No Reddit OAuth credentials are configured. Set REDDTTY_REDDIT_CLIENT_ID and REDDTTY_REDDIT_CLIENT_SECRET, or choose another provider."
                    .into(),
            ),
            notices,
            false,
        );
    };
    let config = RedditConfig {
        client_id: creds.client_id,
        client_secret: creds.client_secret,
        refresh_token: creds.refresh_token,
        username: creds.username,
        user_agent: creds.user_agent,
        base_url: "https://oauth.reddit.com".into(),
        token_url: "https://www.reddit.com/api/v1/access_token".into(),
        request_timeout: Duration::from_secs(20),
    };
    match RedditProvider::new(config) {
        Ok(provider) => {
            let mut notices = notices;
            notices.push(
                "Reddit's public Data API is scheduled to close in March 2027. See docs/API_ACCESS.md."
                    .into(),
            );
            ProviderSetup {
                provider: Arc::new(provider),
                notices,
                demo: false,
            }
        }
        Err(error) => failing("Reddit OAuth", "REDDIT", error, notices, false),
    }
}

fn html_client_config(settings: &Settings, base_url: &str) -> HtmlClientConfig {
    HtmlClientConfig {
        base_url: base_url.to_string(),
        user_agent: settings.reddit.user_agent.clone(),
        allow_network: settings.provider.html.allow_network,
        timeout: html::HTML_TIMEOUT,
    }
}

fn old_reddit_provider(settings: &Settings, notices: Vec<String>) -> ProviderSetup {
    if !settings.provider.html.allow_network {
        return failing(
            "Old Reddit HTML",
            "OLD REDDIT",
            ProviderError::Disabled(
                "Old Reddit HTML network access is disabled. Set provider.html.allow_network = true only after confirming that the source's terms and permissions allow your use."
                    .into(),
            ),
            notices,
            true,
        );
    }
    let config = html_client_config(settings, "https://old.reddit.com");
    match OldRedditProvider::new(config) {
        Ok(provider) => ProviderSetup {
            provider: Arc::new(provider),
            notices,
            demo: false,
        },
        Err(error) => failing("Old Reddit HTML", "OLD REDDIT", error, notices, true),
    }
}

fn redlib_provider(settings: &Settings, notices: Vec<String>) -> ProviderSetup {
    let instance = settings.provider.html.redlib_instance.trim();
    if instance.is_empty() {
        return failing(
            "Redlib HTML",
            "REDLIB",
            ProviderError::InvalidInput(
                "provider.type = \"redlib\" requires provider.html.redlib_instance. ReddTTY never picks a random instance."
                    .into(),
            ),
            notices,
            true,
        );
    }
    if !settings.provider.html.allow_network {
        return failing(
            "Redlib HTML",
            "REDLIB",
            ProviderError::Disabled(
                "Redlib network access is disabled. Set provider.html.allow_network = true only after confirming the instance's terms and permissions."
                    .into(),
            ),
            notices,
            true,
        );
    }
    let config = html_client_config(settings, instance);
    match RedlibProvider::new(config) {
        Ok(provider) => ProviderSetup {
            provider: Arc::new(provider),
            notices,
            demo: false,
        },
        Err(error) => failing("Redlib HTML", "REDLIB", error, notices, true),
    }
}

pub fn build_provider(settings: &Settings, force_demo: bool) -> ProviderSetup {
    let mut notices = Vec::new();
    let kind = if force_demo {
        ProviderKind::Demo
    } else {
        settings.provider.kind
    };

    match kind {
        ProviderKind::Demo => demo_provider(notices),
        ProviderKind::Reddit => reddit_provider(settings, notices),
        ProviderKind::OldReddit => {
            notices.push(policy_note("Old Reddit"));
            old_reddit_provider(settings, notices)
        }
        ProviderKind::Redlib => {
            notices.push(policy_note("Redlib"));
            redlib_provider(settings, notices)
        }
        ProviderKind::Auto => {
            if settings.reddit_credentials().is_some() {
                reddit_provider(settings, notices)
            } else {
                notices.push(
                    "No Reddit credentials configured. Running with synthetic demo data — choose --provider or set provider.type explicitly for an HTML source."
                        .into(),
                );
                demo_provider(notices)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(kind: ProviderKind) -> Settings {
        let mut settings = Settings::default();
        settings.provider.kind = kind;
        settings
    }

    #[tokio::test]
    async fn demo_selection_uses_demo_provider() {
        let setup = build_provider(&settings_with(ProviderKind::Demo), false);
        assert!(setup.demo);
        assert_eq!(setup.provider.short_name(), "DEMO");
        assert!(
            setup
                .provider
                .get_feed(FeedRequest::first(
                    crate::models::FeedSpec::Home,
                    crate::models::FeedSort::Hot,
                    crate::models::TopRange::All,
                    10
                ))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn explicit_reddit_without_credentials_does_not_fall_back_to_demo() {
        let mut settings = settings_with(ProviderKind::Reddit);
        settings.reddit.use_environment = false;
        settings.reddit.client_id.clear();
        settings.reddit.client_secret.clear();
        let setup = build_provider(&settings, false);
        assert!(!setup.demo);
        assert_eq!(setup.provider.short_name(), "REDDIT");
        assert!(matches!(
            setup
                .provider
                .get_feed(FeedRequest::first(
                    crate::models::FeedSpec::Home,
                    crate::models::FeedSort::Hot,
                    crate::models::TopRange::All,
                    10
                ))
                .await,
            Err(ProviderError::AuthRequired(_))
        ));
    }

    #[tokio::test]
    async fn html_network_is_off_by_default_and_never_falls_back() {
        let setup = build_provider(&settings_with(ProviderKind::OldReddit), false);
        assert!(!setup.demo);
        assert_eq!(setup.provider.short_name(), "OLD REDDIT");
        assert!(matches!(
            setup
                .provider
                .get_feed(FeedRequest::first(
                    crate::models::FeedSpec::Home,
                    crate::models::FeedSort::Hot,
                    crate::models::TopRange::All,
                    10
                ))
                .await,
            Err(ProviderError::Disabled(_))
        ));
    }

    #[tokio::test]
    async fn changing_selection_builds_a_new_provider_without_reusing_demo() {
        let demo = build_provider(&settings_with(ProviderKind::Demo), false);
        let old = build_provider(&settings_with(ProviderKind::OldReddit), false);
        let redlib = build_provider(&settings_with(ProviderKind::Redlib), false);
        assert_eq!(demo.provider.short_name(), "DEMO");
        assert_eq!(old.provider.short_name(), "OLD REDDIT");
        assert_eq!(redlib.provider.short_name(), "REDLIB");
        assert!(!old.demo && !redlib.demo);
    }

    #[tokio::test]
    async fn redlib_requires_an_explicit_instance() {
        let setup = build_provider(&settings_with(ProviderKind::Redlib), false);
        assert!(!setup.demo);
        assert_eq!(setup.provider.short_name(), "REDLIB");
        assert!(matches!(
            setup
                .provider
                .get_feed(FeedRequest::first(
                    crate::models::FeedSpec::Home,
                    crate::models::FeedSort::Hot,
                    crate::models::TopRange::All,
                    10
                ))
                .await,
            Err(ProviderError::InvalidInput(_))
        ));
    }
}
