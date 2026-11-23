use async_trait::async_trait;

use crate::error::ProviderError;
use crate::models::{
    Comment, CommentSort, FeedPage, FeedRequest, FeedSort, FeedSpec, Post, Subreddit,
};

/// Health of the active provider, shown in the top bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStatus {
    Available,
    Unavailable,
    AccessDenied,
    LoginRequired,
    RateLimited,
    HtmlStructureChanged,
    ParseError,
    UnsupportedFeature,
    AccessDisabled,
}

impl ProviderStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Unavailable => "Unavailable",
            Self::AccessDenied => "Access denied",
            Self::LoginRequired => "Login required",
            Self::RateLimited => "Rate limited",
            Self::HtmlStructureChanged => "HTML changed",
            Self::ParseError => "Parse error",
            Self::UnsupportedFeature => "Unsupported",
            Self::AccessDisabled => "Access disabled",
        }
    }

    pub fn from_error(error: &ProviderError) -> Self {
        match error {
            ProviderError::AuthRequired(_) | ProviderError::LoginRequired(_) => Self::LoginRequired,
            ProviderError::Forbidden(_) => Self::AccessDenied,
            ProviderError::RateLimited { .. } => Self::RateLimited,
            ProviderError::HtmlStructureChanged(_) => Self::HtmlStructureChanged,
            ProviderError::Parse(_) => Self::ParseError,
            ProviderError::Unsupported(_) => Self::UnsupportedFeature,
            ProviderError::Disabled(_) => Self::AccessDisabled,
            ProviderError::Network(_)
            | ProviderError::Timeout(_)
            | ProviderError::Server(..)
            | ProviderError::NotFound(_)
            | ProviderError::InvalidInput(_)
            | ProviderError::Other(_) => Self::Unavailable,
        }
    }
}

/// What a provider can do. The UI uses this to avoid offering dead actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub search: bool,
    pub search_in_subreddit: bool,
    pub saved: bool,
    pub images: bool,
    pub oauth: bool,
    pub demo: bool,
}

/// The UI depends only on this trait, never on Reddit's wire format.
#[async_trait]
pub trait ContentProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Short uppercase label for the top bar (for example `OLD REDDIT`).
    fn short_name(&self) -> &'static str {
        "LIVE"
    }

    fn is_demo(&self) -> bool;

    fn capabilities(&self) -> ProviderCapabilities;

    /// Sorts supported by this provider for a given feed. Defaults to the
    /// documented Reddit capability matrix.
    fn supported_sorts(&self, spec: &FeedSpec) -> Vec<FeedSort> {
        spec.sort_options()
    }

    /// Map a logical media URL to something the image downloader understands.
    /// The demo provider uses this to point at local fixture files.
    fn resolve_media_url(&self, url: &str) -> String {
        url.to_string()
    }

    async fn get_feed(&self, request: FeedRequest) -> Result<FeedPage, ProviderError>;

    async fn get_post(&self, post_id: &str) -> Result<Post, ProviderError>;

    async fn get_comments(
        &self,
        post_id: &str,
        sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError>;

    async fn get_subreddit(&self, name: &str) -> Result<Subreddit, ProviderError>;

    async fn get_more_comments(
        &self,
        _post_id: &str,
        _children: &[String],
        _sort: CommentSort,
    ) -> Result<Vec<Comment>, ProviderError> {
        Err(ProviderError::Unsupported(
            "loading additional comment branches".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_errors_map_to_top_bar_statuses() {
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::Forbidden("blocked".into())),
            ProviderStatus::AccessDenied
        );
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::LoginRequired("sign in".into())),
            ProviderStatus::LoginRequired
        );
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::HtmlStructureChanged("changed".into())),
            ProviderStatus::HtmlStructureChanged
        );
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::RateLimited {
                retry_after: Some(3)
            }),
            ProviderStatus::RateLimited
        );
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::Unsupported("no".into())),
            ProviderStatus::UnsupportedFeature
        );
        assert_eq!(
            ProviderStatus::from_error(&ProviderError::Disabled("off".into())),
            ProviderStatus::AccessDisabled
        );
        assert_eq!(ProviderStatus::AccessDenied.label(), "Access denied");
        assert_eq!(ProviderStatus::HtmlStructureChanged.label(), "HTML changed");
    }
}
